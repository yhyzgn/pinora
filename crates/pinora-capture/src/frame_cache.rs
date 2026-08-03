//! 空闲时后台预截屏，F2 时零等待弹出 overlay。
//!
//! Spectacle 单次约 0.4–0.5s，无法再压；用「始终备好一帧」换瞬时响应。
//! 帧龄通常 < 截屏间隔（一轮截完立刻再截），观感接近 Snipaste。

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::capture_preview::CapturePreview;
use pinora_core::{
    CaptureImage, CaptureProvider, CaptureRequest, DisplayId, DisplayInfo, PixelPoint,
};

/// 已转好 XRGB + 暗化底的一帧。
pub struct CachedFrame {
    pub image: CaptureImage,
    pub base: Vec<u32>,
    pub dimmed: Vec<u32>,
    pub display_id: DisplayId,
    pub display_origin: PixelPoint,
    pub captured_at: Instant,
}

impl CachedFrame {
    pub fn age(&self) -> Duration {
        self.captured_at.elapsed()
    }

    /// 移交预截帧的像素与显示器信息所有权，避免对整屏缓冲做复制。
    pub fn into_preview_with_display(self) -> (CapturePreview, DisplayId, PixelPoint) {
        let Self {
            image,
            base,
            dimmed,
            display_id,
            display_origin,
            captured_at: _,
        } = self;
        (
            CapturePreview::from_parts(image, base, dimmed),
            display_id,
            display_origin,
        )
    }
}

enum Cmd {
    Stop,
}

struct CacheState {
    latest: Option<CachedFrame>,
    paused: bool,
    generation: u64,
}

/// 后台截屏缓存。
pub struct FrameCache {
    state: Arc<Mutex<CacheState>>,
    cmd_tx: Sender<Cmd>,
    join: Option<JoinHandle<()>>,
}

impl FrameCache {
    /// 启动后台循环（立即抓第一帧）。
    pub fn start<C>(provider: C) -> Self
    where
        C: CaptureProvider + Clone + Send + 'static,
    {
        let state = Arc::new(Mutex::new(CacheState {
            latest: None,
            paused: false,
            generation: 0,
        }));
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let state_w = Arc::clone(&state);

        let join = thread::Builder::new()
            .name("pinora-frame-cache".into())
            .spawn(move || worker(provider, state_w, cmd_rx))
            .ok();

        Self {
            state,
            cmd_tx,
            join,
        }
    }

    pub fn pause(&self) {
        if let Ok(mut state) = self.state.lock() {
            if state.paused {
                return;
            }
            state.paused = true;
            state.generation = state.generation.wrapping_add(1);
            // 暂停之后绝不能把旧桌面或 Overlay 本身作为下一会话的缓存帧。
            state.latest = None;
        }
    }

    pub fn resume(&self) {
        if let Ok(mut state) = self.state.lock() {
            if !state.paused {
                return;
            }
            state.paused = false;
            state.generation = state.generation.wrapping_add(1);
        }
    }

    pub fn is_ready(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.latest.is_some())
            .unwrap_or(false)
    }

    /// 若帧仍新鲜，将其所有权移交给调用方，避免复制全屏像素缓冲。
    pub fn take_if_fresh(&self, max_age: Duration) -> Option<CachedFrame> {
        let mut state = self.state.lock().ok()?;
        if state
            .latest
            .as_ref()
            .is_some_and(|frame| frame.age() <= max_age)
        {
            state.latest.take()
        } else {
            None
        }
    }

    /// 仅在缓存帧与当前显示器拓扑完全一致且仍新鲜时移交所有权。
    ///
    /// 目标显示器菜单可能在热插拔后过期；绝不把另一台屏幕或旧缩放的缓存帧交给
    /// 新会话。未匹配时保留槽位，调用方可选择冷捕获或按当前暂停语义清理。
    pub fn take_for_display_if_fresh(
        &self,
        display: &DisplayInfo,
        max_age: Duration,
    ) -> Option<CachedFrame> {
        self.take_matching(display, |frame| frame.age() <= max_age)
    }

    /// 有任意帧就移交（启动后第一帧可能略旧，仍远好于再等 0.5s）。
    pub fn take_any(&self) -> Option<CachedFrame> {
        self.state.lock().ok()?.latest.take()
    }

    /// 仅在缓存帧与当前显示器拓扑完全一致时移交所有权，不检查帧龄。
    pub fn take_for_display(&self, display: &DisplayInfo) -> Option<CachedFrame> {
        self.take_matching(display, |_| true)
    }

    fn take_matching(
        &self,
        display: &DisplayInfo,
        predicate: impl FnOnce(&CachedFrame) -> bool,
    ) -> Option<CachedFrame> {
        let mut state = self.state.lock().ok()?;
        if state
            .latest
            .as_ref()
            .is_some_and(|frame| predicate(frame) && frame_matches_display(frame, display))
        {
            state.latest.take()
        } else {
            None
        }
    }
}

impl Drop for FrameCache {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(Cmd::Stop);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn worker<C>(provider: C, state: Arc<Mutex<CacheState>>, cmd_rx: Receiver<Cmd>)
where
    C: CaptureProvider,
{
    loop {
        match cmd_rx.try_recv() {
            Ok(Cmd::Stop) | Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => {}
        }

        let generation = match state.lock() {
            Ok(state) if !state.paused => state.generation,
            Ok(_) => {
                thread::sleep(Duration::from_millis(40));
                continue;
            }
            Err(_) => return,
        };

        let started = Instant::now();
        match grab_one(&provider) {
            Ok(frame) => {
                if publish_if_active(&state, generation, frame) {
                    let ms = started.elapsed().as_secs_f64() * 1000.0;
                    // 只在第一帧或偶尔打印，避免刷屏。
                    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                    let n = N
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        .wrapping_add(1);
                    if n == 1 || n.is_multiple_of(20) {
                        let dimensions = state.lock().ok().and_then(|state| {
                            state.latest.as_ref().map(|frame| frame.image.pixels.size)
                        });
                        if let Some(size) = dimensions {
                            println!(
                                "pinora: frame-cache ready {}x{} in {:.0}ms (#{n})",
                                size.width, size.height, ms
                            );
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("pinora: frame-cache grab failed: {e}");
                thread::sleep(Duration::from_millis(300));
            }
        }
        // 截屏本身已耗时；略歇一口气再抓，避免占满 CPU
        thread::sleep(Duration::from_millis(30));
    }
}

fn publish_if_active(state: &Mutex<CacheState>, generation: u64, frame: CachedFrame) -> bool {
    let Ok(mut state) = state.lock() else {
        return false;
    };
    if state.paused || state.generation != generation {
        return false;
    }
    state.latest = Some(frame);
    true
}

fn grab_one(provider: &impl CaptureProvider) -> Result<CachedFrame, String> {
    let displays = provider.displays().map_err(|e| e.to_string())?;
    let display = pick_display(&displays).ok_or_else(|| "no display".to_string())?;
    let image = provider
        .capture(CaptureRequest::FullDisplay {
            display: display.id.clone(),
        })
        .map_err(|e| e.to_string())?;

    let preview = CapturePreview::from_image(image);
    Ok(CachedFrame {
        image: preview.image,
        base: preview.base,
        dimmed: preview.dimmed,
        display_id: display.id,
        display_origin: display.bounds.origin,
        captured_at: Instant::now(),
    })
}

fn pick_display(displays: &[DisplayInfo]) -> Option<DisplayInfo> {
    displays
        .iter()
        .max_by_key(|d| d.bounds.size.area())
        .cloned()
}

fn frame_matches_display(frame: &CachedFrame, display: &DisplayInfo) -> bool {
    frame.display_id == display.id
        && frame.display_origin == display.bounds.origin
        && frame.image.source_rect == display.bounds
        && frame.image.metadata.display == display.id
        && frame.image.metadata.scale == display.scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{CaptureMetadata, ImageId, PixelRect, PixelSize, RgbaBuffer};

    fn sample_frame() -> CachedFrame {
        let display_id = DisplayId::new("test-display");
        let image = CaptureImage::new(
            ImageId::new(),
            RgbaBuffer::new(PixelSize::new(2, 1), vec![1, 2, 3, 255, 4, 5, 6, 255]).unwrap(),
            PixelRect::new(0, 0, 2, 1),
            CaptureMetadata::new(display_id.clone(), 1.0, 0),
        )
        .unwrap();
        CachedFrame {
            image,
            base: vec![0x0001_0203, 0x0004_0506],
            dimmed: vec![0x0000_0001, 0x0000_0002],
            display_id,
            display_origin: PixelPoint::new(0, 0),
            captured_at: Instant::now(),
        }
    }

    fn sample_display() -> DisplayInfo {
        DisplayInfo {
            id: DisplayId::new("test-display"),
            name: "Test display".into(),
            bounds: PixelRect::new(0, 0, 2, 1),
            scale: 1.0,
        }
    }

    fn cache_with(frame: Option<CachedFrame>) -> FrameCache {
        let (cmd_tx, _cmd_rx) = mpsc::channel();
        FrameCache {
            state: Arc::new(Mutex::new(CacheState {
                latest: frame,
                paused: false,
                generation: 0,
            })),
            cmd_tx,
            join: None,
        }
    }

    #[test]
    fn cached_frame_transfers_preview_pixel_ownership() {
        let frame = sample_frame();
        let image_id = frame.image.id;

        let (preview, display_id, display_origin) = frame.into_preview_with_display();

        assert_eq!(preview.image.id, image_id);
        assert_eq!(preview.base, vec![0x0001_0203, 0x0004_0506]);
        assert!(preview.matches_image());
        assert_eq!(display_id, DisplayId::new("test-display"));
        assert_eq!(display_origin, PixelPoint::new(0, 0));
    }

    #[test]
    fn taking_fresh_frame_transfers_ownership_and_clears_cache_slot() {
        let cached = sample_frame();
        let expected = cached.image.id;
        let cache = cache_with(Some(cached));

        let frame = cache.take_if_fresh(Duration::from_secs(1)).unwrap();

        assert_eq!(frame.image.id, expected);
        assert!(!cache.is_ready());
    }

    #[test]
    fn targeted_frame_requires_exact_display_topology() {
        let cache = cache_with(Some(sample_frame()));
        let display = sample_display();

        let frame = cache
            .take_for_display_if_fresh(&display, Duration::from_secs(1))
            .expect("matching display frame");

        assert_eq!(frame.display_id, display.id);
        assert!(!cache.is_ready());
    }

    #[test]
    fn every_display_topology_mismatch_keeps_cached_frame_out_of_the_wrong_session() {
        let mut wrong_id = sample_display();
        wrong_id.id = DisplayId::new("other-display");
        let mut wrong_origin = sample_display();
        wrong_origin.bounds = PixelRect::new(200, 0, 2, 1);
        let mut wrong_size = sample_display();
        wrong_size.bounds = PixelRect::new(0, 0, 3, 1);
        let mut wrong_scale = sample_display();
        wrong_scale.scale = 1.25;

        for display in [wrong_id, wrong_origin, wrong_size, wrong_scale] {
            let cache = cache_with(Some(sample_frame()));
            assert!(
                cache
                    .take_for_display_if_fresh(&display, Duration::from_secs(1))
                    .is_none()
            );
            assert!(cache.is_ready());
        }
    }

    #[test]
    fn paused_generation_rejects_late_capture_publish() {
        let cache = cache_with(None);
        let generation = cache.state.lock().unwrap().generation;

        cache.pause();

        assert!(!publish_if_active(&cache.state, generation, sample_frame()));
        assert!(!cache.is_ready());
    }

    #[test]
    fn repeated_resume_does_not_invalidate_an_active_capture() {
        let cache = cache_with(None);
        let generation = cache.state.lock().unwrap().generation;

        cache.resume();

        assert!(publish_if_active(&cache.state, generation, sample_frame()));
        assert!(cache.is_ready());
    }
}
