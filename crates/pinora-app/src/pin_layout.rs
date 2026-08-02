//! 贴图窗口的纯尺寸计算。
//!
//! 此模块不持有窗口、事件循环或平台资源，因此可由受 tray 监督的桌面壳安全复用。

use pinora_core::PixelSize;

/// 根据图像尺寸与缩放计算窗口物理像素大小。
pub fn scaled_window_size(image: PixelSize, scale: f64) -> (u32, u32) {
    let s = if scale.is_finite() && scale > 0.0 {
        scale.clamp(0.05, 8.0)
    } else {
        1.0
    };
    let w = ((f64::from(image.width) * s).round() as u32).max(1);
    let h = ((f64::from(image.height) * s).round() as u32).max(1);
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_window_size_basic() {
        let (w, h) = scaled_window_size(PixelSize::new(100, 50), 2.0);
        assert_eq!((w, h), (200, 100));
    }

    #[test]
    fn scaled_window_size_min_clamp() {
        let (w, h) = scaled_window_size(PixelSize::new(100, 50), 0.01);
        assert_eq!((w, h), (5, 3));
    }
}
