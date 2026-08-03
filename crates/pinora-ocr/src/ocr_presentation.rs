//! OCR 词框的纯视觉状态。
//!
//! 这里不修改 OCR 结果或选择集合。阈值只决定既有词框的呈现，使复制与缓存始终
//! 保留完整的识别结果。

use pinora_core::OcrWord;

const OCR_WORD_NORMAL_COLOR: u32 = 0x00_22_EE_66;
const OCR_WORD_LOW_CONFIDENCE_COLOR: u32 = 0x00_FF_5A_36;
const OCR_WORD_SELECTED_COLOR: u32 = 0x00_FF_B0_20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrWordVisualState {
    Normal,
    LowConfidence,
    Selected,
}

impl OcrWordVisualState {
    pub const fn color(self) -> u32 {
        match self {
            Self::Normal => OCR_WORD_NORMAL_COLOR,
            Self::LowConfidence => OCR_WORD_LOW_CONFIDENCE_COLOR,
            Self::Selected => OCR_WORD_SELECTED_COLOR,
        }
    }
}

/// 返回与 OCR 原始数据无关的绘制状态。`-1`、NaN、无穷和范围外数值表示未知或
/// 非规范结果，不能被伪装成低置信；选中状态优先，保证拖选反馈稳定。
pub fn word_visual_state(
    word: &OcrWord,
    confidence_threshold: u8,
    selected: bool,
) -> OcrWordVisualState {
    if selected {
        OcrWordVisualState::Selected
    } else if is_low_confidence(word.confidence, confidence_threshold) {
        OcrWordVisualState::LowConfidence
    } else {
        OcrWordVisualState::Normal
    }
}

fn is_low_confidence(confidence: f32, threshold: u8) -> bool {
    confidence.is_finite() && (0.0..=100.0).contains(&confidence) && confidence < threshold as f32
}

#[cfg(test)]
mod tests {
    use pinora_core::{OcrWord, PixelRect};

    use super::{OcrWordVisualState, word_visual_state};

    fn word(confidence: f32) -> OcrWord {
        OcrWord {
            text: "word".into(),
            confidence,
            bbox: PixelRect::new(0, 0, 1, 1),
        }
    }

    #[test]
    fn only_known_values_strictly_below_threshold_are_low_confidence() {
        assert_eq!(
            word_visual_state(&word(59.9), 60, false),
            OcrWordVisualState::LowConfidence
        );
        assert_eq!(
            word_visual_state(&word(60.0), 60, false),
            OcrWordVisualState::Normal
        );
        assert_eq!(
            word_visual_state(&word(0.0), 0, false),
            OcrWordVisualState::Normal
        );
        assert_eq!(
            word_visual_state(&word(99.9), 100, false),
            OcrWordVisualState::LowConfidence
        );
    }

    #[test]
    fn unknown_and_non_finite_values_remain_neutral() {
        for confidence in [-1.0, 100.1, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                word_visual_state(&word(confidence), 100, false),
                OcrWordVisualState::Normal
            );
        }
    }

    #[test]
    fn selected_words_remain_selected_even_when_low_confidence() {
        assert_eq!(
            word_visual_state(&word(1.0), 60, true),
            OcrWordVisualState::Selected
        );
    }
}
