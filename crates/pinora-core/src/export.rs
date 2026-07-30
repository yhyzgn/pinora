//! 图像导出与剪贴板端口（无平台 SDK）。

use std::path::Path;

use crate::error::PinoraError;
use crate::image::CaptureImage;

/// 图像导出与剪贴板抽象。
pub trait ImageSink {
    /// 将图像编码为 PNG 并写入路径。
    fn save_png(&self, image: &CaptureImage, path: &Path) -> Result<(), PinoraError>;

    /// 将图像复制到剪贴板端口（实现可为内存或系统剪贴板）。
    fn copy_image(&self, image: &CaptureImage) -> Result<(), PinoraError>;
}
