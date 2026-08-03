//! 图像导出与剪贴板端口（无平台 SDK）。

use std::path::Path;

use crate::error::PinoraError;
use crate::image::CaptureImage;

/// 文件导出的稳定编码格式。
///
/// 枚举值既是设置 codec 的白名单，也是文件命名与编码分支的唯一来源；不要由
/// 文件扩展名反向推断格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportImageFormat {
    Png,
    Jpeg,
    WebP,
}

impl ExportImageFormat {
    pub const fn to_wire(self) -> u8 {
        match self {
            Self::Png => 0,
            Self::Jpeg => 1,
            Self::WebP => 2,
        }
    }

    pub const fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Png),
            1 => Some(Self::Jpeg),
            2 => Some(Self::WebP),
            _ => None,
        }
    }

    pub const fn file_extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
        }
    }
}

/// 图像导出与剪贴板抽象。
pub trait ImageSink {
    /// 将图像编码为 PNG 并写入路径。
    fn save_png(&self, image: &CaptureImage, path: &Path) -> Result<(), PinoraError>;

    /// 将图像复制到剪贴板端口（实现可为内存或系统剪贴板）。
    fn copy_image(&self, image: &CaptureImage) -> Result<(), PinoraError>;
}

#[cfg(test)]
mod tests {
    use super::ExportImageFormat;

    #[test]
    fn export_format_wire_values_and_extensions_are_stable() {
        assert_eq!(ExportImageFormat::Png.to_wire(), 0);
        assert_eq!(ExportImageFormat::Jpeg.to_wire(), 1);
        assert_eq!(ExportImageFormat::WebP.to_wire(), 2);
        assert_eq!(ExportImageFormat::from_wire(3), None);
        assert_eq!(ExportImageFormat::Png.file_extension(), "png");
        assert_eq!(ExportImageFormat::Jpeg.file_extension(), "jpg");
        assert_eq!(ExportImageFormat::WebP.file_extension(), "webp");
    }
}
