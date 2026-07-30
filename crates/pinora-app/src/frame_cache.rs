//! 空闲时后台预截屏，F2 时零等待弹出 overlay。
//!
//! Spectacle 单次约 0.4–0.5s，无法再压；用「始终备好一帧」换瞬时响应。
//! 帧龄通常 < 截屏间隔（一轮截完立刻再截），观感接近 Snipaste。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use pinora_core::{
    CaptureImage, CaptureProvider, CaptureRequest, DisplayId, DisplayInfo, PixelPoint,
};

/// 已转好 XRGB + 暗化底的一帧。
#[derive(Clone)]
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
    Pause,
    Resume,
    Stop,
}

/// 后台截屏缓存。
pub struct FrameCache {
    latest: Arc<Mutex<Option<CachedFrame>>>,
    cmd_tx: Sender<Cmd>,
    paused: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl FrameCache {
    /// 启动后台循环（立即抓第一帧）。
    pub fn start<C>(provider: C) -> Self
    where
        C: CaptureProvider + Clone + Send + 'static,
    {
        let latest = Arc::new(Mutex::new(None));
        let paused = Arc::new(AtomicBool::new(false));
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let latest_w = Arc::clone(&latest);
        let paused_w = Arc::clone(&paused);

        let join = thread::Builder::new()
            .name("pinora-frame-cache".into())
            .spawn(move || worker(provider, latest_w, paused_w, cmd_rx))
            .ok();

        Self {
            latest,
            cmd_tx,
            paused,
            join,
        }
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
        let _ = self.cmd_tx.send(Cmd::Pause);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        let _ = self.cmd_tx.send(Cmd::Resume);
    }

    /// 取当前最新帧（不清除）。
    pub fn peek(&self) -> Option<CachedFrame> {
        self.latest.lock().ok().and_then(|g| g.clone())
    }

    /// 若有帧则取出克隆；`max_age` 内才算可用。
    pub fn take_if_fresh(&self, max_age: Duration) -> Option<CachedFrame> {
        let guard = self.latest.lock().ok()?;
        let frame = guard.as_ref()?;
        if frame.age() <= max_age {
            Some(frame.clone())
        } else {
            None
        }
    }

    /// 有任意帧就用（启动后第一帧可能略旧，仍远好于再等 0.5s）。
    pub fn take_any(&self) -> Option<CachedFrame> {
        self.latest.lock().ok().and_then(|g| g.clone())
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

fn worker<C>(
    provider: C,
    latest: Arc<Mutex<Option<CachedFrame>>>,
    paused: Arc<AtomicBool>,
    cmd_rx: Receiver<Cmd>,
) where
    C: CaptureProvider,
{
    loop {
        // 处理命令
        loop {
            match cmd_rx.try_recv() {
                Ok(Cmd::Stop) => return,
                Ok(Cmd::Pause) => paused.store(true, Ordering::SeqCst),
                Ok(Cmd::Resume) => paused.store(false, Ordering::SeqCst),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        if paused.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(40));
            continue;
        }

        let started = Instant::now();
        match grab_one(&provider) {
            Ok(frame) => {
                let ms = started.elapsed().as_secs_f64() * 1000.0;
                // 只在第一帧或偶尔打印，避免刷屏
                static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                let n = N.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
                if n == 1 || n % 20 == 0 {
                    println!(
                        "pinora: frame-cache ready {}x{} in {:.0}ms (#{n})",
                        frame.image.pixels.size.width,
                        frame.image.pixels.size.height,
                        ms
                    );
                }
                if let Ok(mut g) = latest.lock() {
                    *g = Some(frame);
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

    #[test]
    fn rgba_dim_len() {
        let bytes = vec![255u8, 0, 0, 255, 0, 255, 0, 255];
        let (b, d) = rgba_to_xrgb_and_dim(&bytes);
        assert_eq!(b.len(), 2);
        assert_eq!(d.len(), 2);
        assert_eq!(b[0], 0x00ff_0000);
    }
}
