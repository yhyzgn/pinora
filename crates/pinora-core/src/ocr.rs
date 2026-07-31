//! OCR 领域模型：词/行/全文结果（图像物理像素坐标）。
//!
//! 不依赖具体 OCR 引擎；引擎在 `pinora-app`。

use crate::geometry::PixelRect;

/// 单个识别词。
#[derive(Debug, Clone, PartialEq)]
pub struct OcrWord {
    pub text: String,
    /// 0..100；引擎未知时为 -1。
    pub confidence: f32,
    pub bbox: PixelRect,
}

/// 一行文字（阅读顺序）。
#[derive(Debug, Clone, PartialEq)]
pub struct OcrLine {
    pub words: Vec<OcrWord>,
    pub bbox: PixelRect,
}

impl OcrLine {
    pub fn text(&self) -> String {
        self.words
            .iter()
            .map(|w| w.text.as_str())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// 一次 OCR 识别结果。
#[derive(Debug, Clone, PartialEq)]
pub struct OcrResult {
    pub lines: Vec<OcrLine>,
    /// 按阅读顺序拼接的全文（行间换行）。
    pub full_text: String,
    pub languages: Vec<String>,
    pub engine: String,
}

impl OcrResult {
    pub fn from_lines(lines: Vec<OcrLine>, languages: Vec<String>, engine: impl Into<String>) -> Self {
        let full_text = join_lines_text(&lines);
        Self {
            lines,
            full_text,
            languages,
            engine: engine.into(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.full_text.trim().is_empty()
    }

    pub fn word_count(&self) -> usize {
        self.lines.iter().map(|l| l.words.len()).sum()
    }
}

/// 将各行文本用换行拼接。
pub fn join_lines_text(lines: &[OcrLine]) -> String {
    lines
        .iter()
        .map(|l| l.text())
        .filter(|t| !t.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 合并多个词框为包围盒；空则 0 尺寸。
pub fn union_bboxes(rects: impl IntoIterator<Item = PixelRect>) -> PixelRect {
    let mut iter = rects.into_iter();
    let Some(first) = iter.next() else {
        return PixelRect::new(0, 0, 0, 0);
    };
    let mut x0 = first.origin.x;
    let mut y0 = first.origin.y;
    let mut x1 = first.right();
    let mut y1 = first.bottom();
    for r in iter {
        x0 = x0.min(r.origin.x);
        y0 = y0.min(r.origin.y);
        x1 = x1.max(r.right());
        y1 = y1.max(r.bottom());
    }
    PixelRect::new(x0, y0, (x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::PixelRect;

    #[test]
    fn join_and_count() {
        let lines = vec![
            OcrLine {
                words: vec![
                    OcrWord {
                        text: "Hello".into(),
                        confidence: 90.0,
                        bbox: PixelRect::new(0, 0, 10, 10),
                    },
                    OcrWord {
                        text: "世界".into(),
                        confidence: 88.0,
                        bbox: PixelRect::new(12, 0, 10, 10),
                    },
                ],
                bbox: PixelRect::new(0, 0, 22, 10),
            },
            OcrLine {
                words: vec![OcrWord {
                    text: "OCR".into(),
                    confidence: 95.0,
                    bbox: PixelRect::new(0, 12, 8, 8),
                }],
                bbox: PixelRect::new(0, 12, 8, 8),
            },
        ];
        let r = OcrResult::from_lines(lines, vec!["eng".into()], "test");
        assert_eq!(r.full_text, "Hello 世界\nOCR");
        assert_eq!(r.word_count(), 3);
        assert!(!r.is_empty());
    }

    #[test]
    fn union_bboxes_works() {
        let u = union_bboxes([
            PixelRect::new(1, 2, 3, 4),
            PixelRect::new(5, 1, 2, 2),
        ]);
        assert_eq!(u.origin.x, 1);
        assert_eq!(u.origin.y, 1);
        assert_eq!(u.right(), 7);
        assert_eq!(u.bottom(), 6);
    }
}
