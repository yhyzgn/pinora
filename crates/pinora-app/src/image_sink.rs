//! 本地 PNG 导出 + 内存剪贴板（非系统剪贴板）。

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Mutex;

use pinora_core::{CaptureImage, ErrorCode, ImageId, ImageSink, PinoraError};

/// 将 RGBA 写为 PNG 文件，并在内存中保存最近一次「复制」的图像。
#[derive(Debug, Default)]
pub struct LocalImageSink {
    clipboard: Mutex<Option<CaptureImage>>,
}

impl LocalImageSink {
    pub fn new() -> Self {
        Self {
            clipboard: Mutex::new(None),
        }
    }

    pub fn clipboard_image_id(&self) -> Option<ImageId> {
        self.clipboard
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|img| img.id))
    }

    pub fn clipboard_byte_len(&self) -> Option<usize> {
        self.clipboard
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|img| img.pixels.byte_len()))
    }
}

impl ImageSink for LocalImageSink {
    fn save_png(&self, image: &CaptureImage, path: &Path) -> Result<(), PinoraError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("create export dir: {e}"))
            })?;
        }
        let file = File::create(path)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("create png: {e}")))?;
        let w = BufWriter::new(file);
        let width = image.pixels.size.width;
        let height = image.pixels.size.height;
        let mut encoder = png::Encoder::new(w, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("png header: {e}"))
        })?;
        writer
            .write_image_data(&image.pixels.bytes)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("png data: {e}")))?;
        Ok(())
    }

    fn copy_image(&self, image: &CaptureImage) -> Result<(), PinoraError> {
        let mut guard = self
            .clipboard
            .lock()
            .map_err(|_| PinoraError::new(ErrorCode::Internal, "clipboard lock poisoned"))?;
        *guard = Some(image.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{
        CaptureMetadata, DisplayId, ImageId, PixelRect, PixelSize, RgbaBuffer,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_image() -> CaptureImage {
        let pixels = RgbaBuffer::solid(PixelSize::new(8, 4), [10, 20, 30, 255]);
        CaptureImage::new(
            ImageId::new(),
            pixels,
            PixelRect::new(0, 0, 8, 4),
            CaptureMetadata::new(DisplayId::new("d0"), 1.0, 0),
        )
        .unwrap()
    }

    #[test]
    fn save_png_writes_signature() {
        let sink = LocalImageSink::new();
        let image = sample_image();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("pinora-export-{nanos}.png"));
        sink.save_png(&image, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn copy_image_tracks_id() {
        let sink = LocalImageSink::new();
        let image = sample_image();
        let id = image.id;
        sink.copy_image(&image).unwrap();
        assert_eq!(sink.clipboard_image_id(), Some(id));
        assert_eq!(sink.clipboard_byte_len(), Some(8 * 4 * 4));
    }
}
