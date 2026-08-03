//! 贴图会话的纯状态和值对象。
//!
//! 贴图窗口、输入、平台命中调用、runtime、OCR、导出、tray 和唯一 EventLoop 仍由
//! `desktop_shell` 持有。

use pinora_core::{CaptureImage, PixelPoint};

/// 贴图窗口请求给平台的鼠标命中状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PinMouseMode {
    Direct,
    Passthrough,
}

impl PinMouseMode {
    pub(crate) const fn hittest_enabled(self) -> bool {
        matches!(self, Self::Direct)
    }
}

/// 平台调用失败时，进程内状态必须保留原值，不能把请求当作已生效。
pub(crate) const fn pin_mouse_mode_after_platform_request(
    current: PinMouseMode,
    requested: PinMouseMode,
    platform_succeeded: bool,
) -> PinMouseMode {
    if platform_succeeded {
        requested
    } else {
        current
    }
}

/// 创建贴图窗口所需的呈现参数。预处理像素仅由受监督的历史读取 worker 提供。
pub(crate) struct PinPresentation {
    pub(crate) position: PixelPoint,
    pub(crate) scale: f64,
    pub(crate) opacity: f64,
    pub(crate) always_on_top: bool,
    pub(crate) pixels_xrgb: Option<Vec<u32>>,
}

/// 可撤销关闭的贴图快照。
///
/// 快照只保留可恢复的领域图像和用户可见呈现参数。窗口、Surface、鼠标穿透和 worker
/// 都属于已结束的窗口生命周期，不能在恢复时携带。
#[derive(Clone)]
pub(crate) struct ClosedPinSnapshot {
    pub(crate) image: CaptureImage,
    pub(crate) position: PixelPoint,
    pub(crate) scale: f64,
    pub(crate) opacity: f64,
    pub(crate) locked: bool,
    pub(crate) always_on_top: bool,
}

impl ClosedPinSnapshot {
    pub(crate) fn new(
        image: CaptureImage,
        position: PixelPoint,
        scale: f64,
        opacity: f64,
        locked: bool,
        always_on_top: bool,
    ) -> Self {
        Self {
            image,
            position,
            scale,
            opacity,
            locked,
            always_on_top,
        }
    }
}

/// 进程内最近使用序号；饱和后保持最大值而不是回绕，避免 tray 排序逆序。
pub(crate) const fn next_pin_recency(current: u64) -> u64 {
    current.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::{
        ClosedPinSnapshot, PinMouseMode, next_pin_recency, pin_mouse_mode_after_platform_request,
    };
    use pinora_core::{
        CaptureImage, CaptureMetadata, DisplayId, ImageId, PixelPoint, PixelRect, PixelSize,
        RgbaBuffer,
    };

    fn image() -> CaptureImage {
        CaptureImage::new(
            ImageId::from_raw(8),
            RgbaBuffer::solid(PixelSize::new(2, 3), [1, 2, 3, 255]),
            PixelRect::new(-4, 5, 2, 3),
            CaptureMetadata::new(DisplayId::new("pin-session"), 1.0, 0),
        )
        .expect("capture image")
    }

    #[test]
    fn mouse_mode_changes_only_after_the_platform_accepts_the_request() {
        assert!(PinMouseMode::Direct.hittest_enabled());
        assert!(!PinMouseMode::Passthrough.hittest_enabled());
        assert_eq!(
            pin_mouse_mode_after_platform_request(
                PinMouseMode::Direct,
                PinMouseMode::Passthrough,
                true,
            ),
            PinMouseMode::Passthrough
        );
        assert_eq!(
            pin_mouse_mode_after_platform_request(
                PinMouseMode::Direct,
                PinMouseMode::Passthrough,
                false,
            ),
            PinMouseMode::Direct
        );
        assert_eq!(
            pin_mouse_mode_after_platform_request(
                PinMouseMode::Passthrough,
                PinMouseMode::Direct,
                false,
            ),
            PinMouseMode::Passthrough
        );
    }

    #[test]
    fn recency_counter_is_monotonic_and_saturates() {
        assert_eq!(next_pin_recency(0), 1);
        assert_eq!(next_pin_recency(41), 42);
        assert_eq!(next_pin_recency(u64::MAX), u64::MAX);
    }

    #[test]
    fn close_snapshot_keeps_only_restorable_presentation_fields() {
        let image = image();
        let snapshot = ClosedPinSnapshot::new(
            image.clone(),
            PixelPoint::new(-12, 34),
            1.5,
            0.7,
            true,
            false,
        );

        assert_eq!(snapshot.image, image);
        assert_eq!(snapshot.position, PixelPoint::new(-12, 34));
        assert_eq!(snapshot.scale, 1.5);
        assert_eq!(snapshot.opacity, 0.7);
        assert!(snapshot.locked);
        assert!(!snapshot.always_on_top);
    }
}
