//! Pinora 本地 OCR 边界：tesseract CLI 适配、TSV 解析和词框视觉状态。
//!
//! 本 crate 不拥有任务 owner、窗口或剪贴板；调用方负责通过 `pinora-jobs` 管理
//! 生命周期并把结果交付到仍然有效的资产。

mod ocr;
mod ocr_presentation;

pub use ocr::{
    recognize_image, recognize_image_with_cancellation, recognize_image_with_language,
    tesseract_available,
};
pub use ocr_presentation::{OcrWordVisualState, word_visual_state};
