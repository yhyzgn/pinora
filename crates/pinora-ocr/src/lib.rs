//! Pinora OCR 边界：本地 tesseract 适配、受监督 OCR 服务和词框视觉状态。
//!
//! 本 crate 不拥有窗口、剪贴板或应用 EventLoop；任务 owner、资产代际、截止时间
//! 和取消门禁由 `OcrJobService` 与 `pinora-jobs` 协作处理，调用方负责把已验收结果
//! 交付到仍然有效的 UI 资产。

mod job;
mod ocr;
mod ocr_presentation;

pub use job::{LocalOcrRunner, OcrJobCompletion, OcrJobService, OcrJobStart, OcrRunner};
pub use ocr::{
    recognize_image, recognize_image_with_cancellation, recognize_image_with_language,
    tesseract_available,
};
pub use ocr_presentation::{OcrWordVisualState, word_visual_state};
