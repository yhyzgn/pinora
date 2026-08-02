//! 空闲时后台预截屏，F2 时零等待弹出 overlay。
//!
//! Spectacle 单次约 0.4–0.5s，无法再压；用「始终备好一帧」换瞬时响应。
//! 帧龄通常 < 截屏间隔（一轮截完立刻再截），观感接近 Snipaste。

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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

    /// 有任意帧就移交（启动后第一帧可能略旧，仍远好于再等 0.5s）。
    pub fn take_any(&self) -> Option<CachedFrame> {
        self.state.lock().ok()?.latest.take()
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

    let (base, dimmed) = rgba_to_xrgb_and_dim(&image.pixels.bytes);
    Ok(CachedFrame {
        image,
        base,
        dimmed,
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

/// 单遍：RGBA → XRGB base + dimmed。
pub fn rgba_to_xrgb_and_dim(bytes: &[u8]) -> (Vec<u32>, Vec<u32>) {
    let n = bytes.len() / 4;
    let mut base = Vec::with_capacity(n);
    let mut dimmed = Vec::with_capacity(n);
    for c in bytes.chunks_exact(4) {
        let r = u32::from(c[0]);
        let g = u32::from(c[1]);
        let b = u32::from(c[2]);
        let p = (r << 16) | (g << 8) | b;
        base.push(p);
        // ≈55% 亮度
        let dr = r * 11 / 20;
        let dg = g * 11 / 20;
        let db = b * 11 / 20;
        dimmed.push((dr << 16) | (dg << 8) | db);
    }
    (base, dimmed)
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
    fn rgba_dim_len() {
        let bytes = vec![255u8, 0, 0, 255, 0, 255, 0, 255];
        let (b, d) = rgba_to_xrgb_and_dim(&bytes);
        assert_eq!(b.len(), 2);
        assert_eq!(d.len(), 2);
        assert_eq!(b[0], 0x00ff_0000);
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
