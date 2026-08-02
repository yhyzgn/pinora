//! 标注：矩形/圆角矩形/箭头/画笔/椭圆/马赛克/文本；颜色与线宽；栅格化到 RGBA。

use std::num::NonZeroU64;
use std::sync::OnceLock;

use crate::geometry::PixelPoint;
use crate::ids::ImageId;
use crate::image::{CaptureImage, RgbaBuffer};

/// 标注工具。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AnnotateTool {
    #[default]
    Rect,
    RoundedRect,
    Line,
    Arrow,
    Pen,
    Ellipse,
    Number,
    Mosaic,
    Text,
    /// 从当前截图像素采样后续标注颜色；不生成标注事务。
    ColorPicker,
}

/// 单条标注（已提交）。
#[derive(Debug, Clone, PartialEq)]
pub enum Annotation {
    Rect {
        a: PixelPoint,
        b: PixelPoint,
        color: [u8; 4],
        stroke: u32,
        /// 提交时冻结的半透明填充；`None` 保持仅描边。
        fill: Option<[u8; 4]>,
    },
    RoundedRect {
        a: PixelPoint,
        b: PixelPoint,
        color: [u8; 4],
        stroke: u32,
        radius: u32,
        /// 提交时冻结的半透明填充；`None` 保持仅描边。
        fill: Option<[u8; 4]>,
    },
    Line {
        from: PixelPoint,
        to: PixelPoint,
        color: [u8; 4],
        stroke: u32,
    },
    Arrow {
        from: PixelPoint,
        to: PixelPoint,
        color: [u8; 4],
        stroke: u32,
    },
    Pen {
        points: Vec<PixelPoint>,
        color: [u8; 4],
        stroke: u32,
    },
    Ellipse {
        a: PixelPoint,
        b: PixelPoint,
        color: [u8; 4],
        stroke: u32,
        /// 提交时冻结的半透明填充；`None` 保持仅描边。
        fill: Option<[u8; 4]>,
    },
    Number {
        center: PixelPoint,
        value: u32,
        color: [u8; 4],
        diameter: u32,
    },
    Mosaic {
        a: PixelPoint,
        b: PixelPoint,
        block: u32,
    },
    Text {
        origin: PixelPoint,
        content: String,
        color: [u8; 4],
        /// 字号（像素高度近似）。
        size: f32,
    },
}

/// 标注文档的单调版本号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnnotationRevision(NonZeroU64);

impl AnnotationRevision {
    pub const INITIAL: Self = Self(NonZeroU64::MIN);

    pub const fn from_raw(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn raw(self) -> u64 {
        self.0.get()
    }

    /// 版本达到最大值后保持最大值，绝不回绕到较早版本。
    pub fn advance(self) -> Self {
        self.0.checked_add(1).map(Self).unwrap_or(self)
    }
}

impl Default for AnnotationRevision {
    fn default() -> Self {
        Self::INITIAL
    }
}

/// 标注文档（图像坐标系）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnnotationDoc {
    items: Vec<Annotation>,
    redo: Vec<Annotation>,
    revision: AnnotationRevision,
}

impl AnnotationDoc {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, item: Annotation) {
        self.redo.clear();
        self.items.push(item);
        self.revision = self.revision.advance();
    }

    pub fn undo(&mut self) -> Option<Annotation> {
        let item = self.items.pop()?;
        self.redo.push(item.clone());
        self.revision = self.revision.advance();
        Some(item)
    }

    /// 恢复最近一次撤销的标注。恢复不经过 `push`，以保留其余 redo 事务。
    pub fn redo(&mut self) -> Option<Annotation> {
        let item = self.redo.pop()?;
        self.items.push(item.clone());
        self.revision = self.revision.advance();
        Some(item)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn items(&self) -> &[Annotation] {
        &self.items
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub const fn revision(&self) -> AnnotationRevision {
        self.revision
    }
}

/// 默认描边色（亮红）。
pub const DEFAULT_STROKE: [u8; 4] = [255, 64, 64, 255];
pub const DEFAULT_WIDTH: u32 = 3;
pub const MIN_STROKE: u32 = 1;
pub const MAX_STROKE: u32 = 24;
pub const MIN_SEQUENCE_NUMBER: u32 = 1;
pub const MAX_SEQUENCE_NUMBER: u32 = 99_999;
const SHAPE_FILL_ALPHA: u8 = 96;

fn shape_fill_color(color: [u8; 4]) -> [u8; 4] {
    [color[0], color[1], color[2], SHAPE_FILL_ALPHA]
}

/// 可循环调色板。
pub const STROKE_PALETTE: [[u8; 4]; 8] = [
    [255, 64, 64, 255],   // 红
    [255, 160, 0, 255],   // 橙
    [255, 220, 0, 255],   // 黄
    [64, 200, 80, 255],   // 绿
    [64, 160, 255, 255],  // 蓝
    [180, 80, 255, 255],  // 紫
    [255, 255, 255, 255], // 白
    [32, 32, 32, 255],    // 近黑
];

/// 进行中的拖拽/输入草稿。
#[derive(Debug, Clone, PartialEq)]
pub enum DraftShape {
    Rect { a: PixelPoint, b: PixelPoint },
    RoundedRect { a: PixelPoint, b: PixelPoint },
    Line { from: PixelPoint, to: PixelPoint },
    Arrow { from: PixelPoint, to: PixelPoint },
    Pen { points: Vec<PixelPoint> },
    Ellipse { a: PixelPoint, b: PixelPoint },
    Mosaic { a: PixelPoint, b: PixelPoint },
    Text { origin: PixelPoint, content: String },
}

/// 标注编辑会话（纯逻辑）。
#[derive(Debug, Clone)]
pub struct AnnotateSession {
    pub tool: AnnotateTool,
    pub doc: AnnotationDoc,
    pub draft: Option<DraftShape>,
    pub color: [u8; 4],
    pub color_index: usize,
    pub stroke: u32,
    pub image_w: u32,
    pub image_h: u32,
    shape_fill_enabled: bool,
    next_sequence_number: u32,
    sequence_exhausted: bool,
}

impl AnnotateSession {
    pub fn new(image_w: u32, image_h: u32) -> Self {
        Self {
            tool: AnnotateTool::Rect,
            doc: AnnotationDoc::new(),
            draft: None,
            color: DEFAULT_STROKE,
            color_index: 0,
            stroke: DEFAULT_WIDTH,
            image_w: image_w.max(1),
            image_h: image_h.max(1),
            shape_fill_enabled: false,
            next_sequence_number: MIN_SEQUENCE_NUMBER,
            sequence_exhausted: false,
        }
    }

    pub fn clamp_point(&self, p: PixelPoint) -> PixelPoint {
        PixelPoint::new(
            p.x.clamp(0, self.image_w.saturating_sub(1) as i32),
            p.y.clamp(0, self.image_h.saturating_sub(1) as i32),
        )
    }

    /// 循环下一颜色。
    pub fn cycle_color(&mut self) {
        self.color_index = (self.color_index + 1) % STROKE_PALETTE.len();
        self.color = STROKE_PALETTE[self.color_index];
    }

    /// 设置后续标注颜色。取色本身不改变文档或其 revision。
    pub fn set_color(&mut self, color: [u8; 4]) {
        self.color = color;
        if let Some(index) = STROKE_PALETTE
            .iter()
            .position(|candidate| *candidate == color)
        {
            self.color_index = index;
        }
    }

    pub fn stroke_up(&mut self) {
        self.stroke = (self.stroke + 1).min(MAX_STROKE);
    }

    pub fn stroke_down(&mut self) {
        self.stroke = self.stroke.saturating_sub(1).max(MIN_STROKE);
    }

    /// 切换后续封闭图形的半透明填充；这是会话样式，不会改写文档。
    pub fn toggle_shape_fill(&mut self) -> bool {
        self.shape_fill_enabled = !self.shape_fill_enabled;
        self.shape_fill_enabled
    }

    pub const fn shape_fill_enabled(&self) -> bool {
        self.shape_fill_enabled
    }

    /// 设置下一枚序号的起始值。值受显示与布局上限约束，不改写既有标注。
    pub fn set_sequence_start(&mut self, value: u32) -> u32 {
        let value = value.clamp(MIN_SEQUENCE_NUMBER, MAX_SEQUENCE_NUMBER);
        self.next_sequence_number = value;
        self.sequence_exhausted = false;
        value
    }

    /// 下一枚将被提交的序号；达到上限后返回 `None`，直到重新设置起始值。
    pub const fn next_sequence_number(&self) -> Option<u32> {
        if self.sequence_exhausted {
            None
        } else {
            Some(self.next_sequence_number)
        }
    }

    /// 是否正在编辑文本草稿（键入中）。
    pub fn is_text_editing(&self) -> bool {
        matches!(self.draft, Some(DraftShape::Text { .. }))
    }

    pub fn begin(&mut self, p: PixelPoint) {
        let p = self.clamp_point(p);
        if self.tool == AnnotateTool::ColorPicker {
            return;
        }
        if self.tool == AnnotateTool::Number {
            let Some(value) = self.next_sequence_number() else {
                return;
            };
            let diameter = sequence_marker_diameter(self.stroke, value);
            self.doc.push(Annotation::Number {
                center: p,
                value,
                color: self.color,
                diameter,
            });
            if value == MAX_SEQUENCE_NUMBER {
                self.sequence_exhausted = true;
            } else {
                self.next_sequence_number = value + 1;
            }
            return;
        }
        // 文本：若已在编辑中则先提交再在新位置开始
        if self.tool == AnnotateTool::Text {
            if self.is_text_editing() {
                self.commit();
            }
            self.draft = Some(DraftShape::Text {
                origin: p,
                content: String::new(),
            });
            return;
        }
        self.draft = Some(match self.tool {
            AnnotateTool::Rect => DraftShape::Rect { a: p, b: p },
            AnnotateTool::RoundedRect => DraftShape::RoundedRect { a: p, b: p },
            AnnotateTool::Line => DraftShape::Line { from: p, to: p },
            AnnotateTool::Arrow => DraftShape::Arrow { from: p, to: p },
            AnnotateTool::Pen => DraftShape::Pen { points: vec![p] },
            AnnotateTool::Ellipse => DraftShape::Ellipse { a: p, b: p },
            AnnotateTool::Mosaic => DraftShape::Mosaic { a: p, b: p },
            AnnotateTool::Text => unreachable!(),
            AnnotateTool::Number | AnnotateTool::ColorPicker => return,
        });
    }

    pub fn drag(&mut self, p: PixelPoint) {
        let p = self.clamp_point(p);
        match &mut self.draft {
            Some(DraftShape::Rect { b, .. }) => *b = p,
            Some(DraftShape::RoundedRect { b, .. }) => *b = p,
            Some(DraftShape::Line { to, .. }) => *to = p,
            Some(DraftShape::Arrow { to, .. }) => *to = p,
            Some(DraftShape::Ellipse { b, .. }) => *b = p,
            Some(DraftShape::Mosaic { b, .. }) => *b = p,
            Some(DraftShape::Pen { points }) => {
                if let Some(last) = points.last().copied() {
                    let dx = p.x - last.x;
                    let dy = p.y - last.y;
                    let dist = ((dx * dx + dy * dy) as f64).sqrt();
                    if dist >= 0.5 {
                        let steps = (dist / 1.5).ceil() as i32;
                        for s in 1..=steps {
                            let t = s as f64 / steps as f64;
                            let ix = last.x + (dx as f64 * t).round() as i32;
                            let iy = last.y + (dy as f64 * t).round() as i32;
                            let np = PixelPoint::new(ix, iy);
                            if points.last().copied() != Some(np) {
                                points.push(np);
                            }
                        }
                    }
                } else {
                    points.push(p);
                }
            }
            Some(DraftShape::Text { .. }) | None => {}
        }
    }

    /// 文本草稿追加字符（IME commit / 键盘字符）。
    pub fn text_push(&mut self, s: &str) {
        if let Some(DraftShape::Text { content, .. }) = &mut self.draft {
            content.push_str(s);
        }
    }

    /// 文本草稿退格一个 Unicode 标量。
    pub fn text_backspace(&mut self) {
        if let Some(DraftShape::Text { content, .. }) = &mut self.draft {
            content.pop();
        }
    }

    pub fn commit(&mut self) {
        let Some(draft) = self.draft.take() else {
            return;
        };
        let color = self.color;
        let stroke = self.stroke.max(1);
        let fill = self.shape_fill_enabled.then_some(shape_fill_color(color));
        let item = match draft {
            DraftShape::Rect { a, b } => {
                if (a.x - b.x).abs() < 2 && (a.y - b.y).abs() < 2 {
                    return;
                }
                Annotation::Rect {
                    a,
                    b,
                    color,
                    stroke,
                    fill,
                }
            }
            DraftShape::RoundedRect { a, b } => {
                if (a.x - b.x).abs() < 2 || (a.y - b.y).abs() < 2 {
                    return;
                }
                Annotation::RoundedRect {
                    a,
                    b,
                    color,
                    stroke,
                    radius: rounded_rect_radius(a, b, stroke),
                    fill,
                }
            }
            DraftShape::Line { from, to } => {
                if (from.x - to.x).abs() < 2 && (from.y - to.y).abs() < 2 {
                    return;
                }
                Annotation::Line {
                    from,
                    to,
                    color,
                    stroke,
                }
            }
            DraftShape::Arrow { from, to } => {
                if (from.x - to.x).abs() < 2 && (from.y - to.y).abs() < 2 {
                    return;
                }
                Annotation::Arrow {
                    from,
                    to,
                    color,
                    stroke,
                }
            }
            DraftShape::Pen { points } => {
                if points.len() < 2 {
                    return;
                }
                Annotation::Pen {
                    points,
                    color,
                    stroke,
                }
            }
            DraftShape::Ellipse { a, b } => {
                if (a.x - b.x).abs() < 2 && (a.y - b.y).abs() < 2 {
                    return;
                }
                Annotation::Ellipse {
                    a,
                    b,
                    color,
                    stroke,
                    fill,
                }
            }
            DraftShape::Mosaic { a, b } => {
                if (a.x - b.x).abs() < 2 && (a.y - b.y).abs() < 2 {
                    return;
                }
                let block = (stroke * 2).clamp(4, 32);
                Annotation::Mosaic { a, b, block }
            }
            DraftShape::Text { origin, content } => {
                let content = content.trim().to_string();
                if content.is_empty() {
                    return;
                }
                let size = (12.0 + stroke as f32 * 4.0).clamp(12.0, 72.0);
                Annotation::Text {
                    origin,
                    content,
                    color,
                    size,
                }
            }
        };
        self.doc.push(item);
    }

    pub fn cancel_draft(&mut self) {
        self.draft = None;
    }
}

/// 读取一张截图中单个像素的 RGBA 分量。无效坐标或不完整缓冲返回 `None`。
pub fn sample_rgba_at(image: &CaptureImage, point: PixelPoint) -> Option<[u8; 4]> {
    if point.x < 0 || point.y < 0 {
        return None;
    }
    let x = usize::try_from(point.x).ok()?;
    let y = usize::try_from(point.y).ok()?;
    let width = image.pixels.size.width as usize;
    let height = image.pixels.size.height as usize;
    if x >= width || y >= height {
        return None;
    }
    let index = y.checked_mul(width)?.checked_add(x)?.checked_mul(4)?;
    Some([
        *image.pixels.bytes.get(index)?,
        *image.pixels.bytes.get(index + 1)?,
        *image.pixels.bytes.get(index + 2)?,
        *image.pixels.bytes.get(index + 3)?,
    ])
}

/// 取色器复制给用户的稳定不透明颜色文本。Alpha 只用于后续绘制，不进入 HEX 文本。
pub fn color_to_hex(color: [u8; 4]) -> String {
    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}

/// 将标注烧录到新图像（不修改源图）。
pub fn bake_annotations(source: &CaptureImage, doc: &AnnotationDoc) -> CaptureImage {
    let mut bytes = source.pixels.bytes.clone();
    let w = source.pixels.size.width as i32;
    let h = source.pixels.size.height as i32;
    // 马赛克需要源像素；先 clone 一份只读源
    let src_bytes = source.pixels.bytes.clone();
    for item in doc.items() {
        match item {
            Annotation::Rect {
                a,
                b,
                color,
                stroke,
                fill,
            } => {
                if let Some(fill) = fill {
                    draw_rect_fill(&mut bytes, w, h, *a, *b, *fill);
                }
                draw_rect_outline(&mut bytes, w, h, *a, *b, *color, *stroke);
            }
            Annotation::RoundedRect {
                a,
                b,
                color,
                stroke,
                radius,
                fill,
            } => {
                let geometry = RoundedRectGeometry {
                    a: *a,
                    b: *b,
                    radius: *radius,
                };
                if let Some(fill) = fill {
                    draw_rounded_rect_fill(&mut bytes, w, h, geometry, *fill);
                }
                draw_rounded_rect_outline(&mut bytes, w, h, geometry, *color, *stroke);
            }
            Annotation::Line {
                from,
                to,
                color,
                stroke,
            } => draw_line(&mut bytes, w, h, *from, *to, *color, *stroke),
            Annotation::Arrow {
                from,
                to,
                color,
                stroke,
            } => draw_arrow(&mut bytes, w, h, *from, *to, *color, *stroke),
            Annotation::Pen {
                points,
                color,
                stroke,
            } => draw_polyline(&mut bytes, w, h, points, *color, *stroke),
            Annotation::Ellipse {
                a,
                b,
                color,
                stroke,
                fill,
            } => {
                if let Some(fill) = fill {
                    draw_ellipse_fill(&mut bytes, w, h, *a, *b, *fill);
                }
                draw_ellipse_outline(&mut bytes, w, h, *a, *b, *color, *stroke);
            }
            Annotation::Number {
                center,
                value,
                color,
                diameter,
            } => draw_sequence_marker(&mut bytes, w, h, *center, *value, *color, *diameter),
            Annotation::Mosaic { a, b, block } => {
                draw_mosaic(&mut bytes, &src_bytes, w, h, *a, *b, *block)
            }
            Annotation::Text {
                origin,
                content,
                color,
                size,
            } => draw_text(&mut bytes, w, h, *origin, content, *color, *size),
        }
    }
    let pixels = RgbaBuffer {
        size: source.pixels.size,
        bytes,
    };
    CaptureImage {
        id: ImageId::new(),
        pixels,
        source_rect: source.source_rect,
        metadata: source.metadata.clone(),
    }
}

/// 叠加草稿后的预览缓冲（RGBA）。
pub fn render_preview_rgba(source: &CaptureImage, session: &AnnotateSession) -> Vec<u8> {
    let mut doc = session.doc.clone();
    if let Some(draft) = &session.draft {
        let color = session.color;
        let stroke = session.stroke.max(1);
        let fill = session
            .shape_fill_enabled()
            .then_some(shape_fill_color(color));
        match draft {
            DraftShape::Rect { a, b } => doc.push(Annotation::Rect {
                a: *a,
                b: *b,
                color,
                stroke,
                fill,
            }),
            DraftShape::RoundedRect { a, b } => doc.push(Annotation::RoundedRect {
                a: *a,
                b: *b,
                color,
                stroke,
                radius: rounded_rect_radius(*a, *b, stroke),
                fill,
            }),
            DraftShape::Line { from, to } => doc.push(Annotation::Line {
                from: *from,
                to: *to,
                color,
                stroke,
            }),
            DraftShape::Arrow { from, to } => doc.push(Annotation::Arrow {
                from: *from,
                to: *to,
                color,
                stroke,
            }),
            DraftShape::Pen { points } => doc.push(Annotation::Pen {
                points: points.clone(),
                color,
                stroke,
            }),
            DraftShape::Ellipse { a, b } => doc.push(Annotation::Ellipse {
                a: *a,
                b: *b,
                color,
                stroke,
                fill,
            }),
            DraftShape::Mosaic { a, b } => {
                let block = (stroke * 2).clamp(4, 32);
                doc.push(Annotation::Mosaic {
                    a: *a,
                    b: *b,
                    block,
                });
            }
            DraftShape::Text { origin, content } => {
                // 预览时显示光标
                let mut shown = content.clone();
                shown.push('|');
                let size = (12.0 + stroke as f32 * 4.0).clamp(12.0, 72.0);
                doc.push(Annotation::Text {
                    origin: *origin,
                    content: shown,
                    color,
                    size,
                });
            }
        }
    }
    bake_annotations(source, &doc).pixels.bytes
}

/// 带覆盖率的 alpha over（coverage 0..1）。
fn blend_coverage(buf: &mut [u8], w: i32, h: i32, x: i32, y: i32, color: [u8; 4], cov: f64) {
    if x < 0 || y < 0 || x >= w || y >= h || cov <= 0.0 {
        return;
    }
    let cov = cov.clamp(0.0, 1.0);
    let i = ((y * w + x) * 4) as usize;
    if i + 3 >= buf.len() {
        return;
    }
    let a = ((color[3] as f64) * cov).round() as u32;
    if a == 0 {
        return;
    }
    if a >= 255 {
        buf[i] = color[0];
        buf[i + 1] = color[1];
        buf[i + 2] = color[2];
        buf[i + 3] = 255;
        return;
    }
    let inv = 255 - a;
    buf[i] = ((color[0] as u32 * a + buf[i] as u32 * inv) / 255) as u8;
    buf[i + 1] = ((color[1] as u32 * a + buf[i + 1] as u32 * inv) / 255) as u8;
    buf[i + 2] = ((color[2] as u32 * a + buf[i + 2] as u32 * inv) / 255) as u8;
    buf[i + 3] = 255;
}

/// 点到线段距离（像素）。
fn dist_point_segment(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-12 {
        let ex = px - x0;
        let ey = py - y0;
        return (ex * ex + ey * ey).sqrt();
    }
    let t = ((px - x0) * dx + (py - y0) * dy) / len2;
    let t = t.clamp(0.0, 1.0);
    let qx = x0 + t * dx;
    let qy = y0 + t * dy;
    let ex = px - qx;
    let ey = py - qy;
    (ex * ex + ey * ey).sqrt()
}

/// 抗锯齿粗线：胶囊距离场 + 1px 软边。
fn draw_line(
    buf: &mut [u8],
    w: i32,
    h: i32,
    from: PixelPoint,
    to: PixelPoint,
    color: [u8; 4],
    stroke: u32,
) {
    let x0 = from.x as f64;
    let y0 = from.y as f64;
    let x1 = to.x as f64;
    let y1 = to.y as f64;
    let radius = (stroke as f64 * 0.5).max(0.75);
    let aa = 1.0;
    let pad = (radius + aa + 1.0).ceil() as i32;

    let min_x = (x0.min(x1).floor() as i32 - pad).max(0);
    let max_x = (x0.max(x1).ceil() as i32 + pad).min(w - 1);
    let min_y = (y0.min(y1).floor() as i32 - pad).max(0);
    let max_y = (y0.max(y1).ceil() as i32 + pad).min(h - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let d = dist_point_segment(x as f64 + 0.5, y as f64 + 0.5, x0, y0, x1, y1);
            let cov = ((radius + aa * 0.5 - d) / aa).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_coverage(buf, w, h, x, y, color, cov);
            }
        }
    }
}

/// 抗锯齿圆点（画笔端点 / 补点）。
fn stamp_disc(buf: &mut [u8], w: i32, h: i32, cx: f64, cy: f64, radius: f64, color: [u8; 4]) {
    let aa = 1.0;
    let pad = (radius + aa + 1.0).ceil() as i32;
    let min_x = (cx.floor() as i32 - pad).max(0);
    let max_x = (cx.ceil() as i32 + pad).min(w - 1);
    let min_y = (cy.floor() as i32 - pad).max(0);
    let max_y = (cy.ceil() as i32 + pad).min(h - 1);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f64 + 0.5 - cx;
            let dy = y as f64 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            let cov = ((radius + aa * 0.5 - d) / aa).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_coverage(buf, w, h, x, y, color, cov);
            }
        }
    }
}

fn draw_rect_outline(
    buf: &mut [u8],
    w: i32,
    h: i32,
    a: PixelPoint,
    b: PixelPoint,
    color: [u8; 4],
    stroke: u32,
) {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    draw_line(
        buf,
        w,
        h,
        PixelPoint::new(x0, y0),
        PixelPoint::new(x1, y0),
        color,
        stroke,
    );
    draw_line(
        buf,
        w,
        h,
        PixelPoint::new(x1, y0),
        PixelPoint::new(x1, y1),
        color,
        stroke,
    );
    draw_line(
        buf,
        w,
        h,
        PixelPoint::new(x1, y1),
        PixelPoint::new(x0, y1),
        color,
        stroke,
    );
    draw_line(
        buf,
        w,
        h,
        PixelPoint::new(x0, y1),
        PixelPoint::new(x0, y0),
        color,
        stroke,
    );
}

/// 矩形填充：边界仍交给随后描边覆盖，内部和边缘均使用同一 alpha-over 合成。
fn draw_rect_fill(buf: &mut [u8], w: i32, h: i32, a: PixelPoint, b: PixelPoint, color: [u8; 4]) {
    if w <= 0 || h <= 0 {
        return;
    }
    let x0 = a.x.min(b.x).max(0);
    let y0 = a.y.min(b.y).max(0);
    let x1 = a.x.max(b.x).min(w - 1);
    let y1 = a.y.max(b.y).min(h - 1);
    if x1 < x0 || y1 < y0 {
        return;
    }
    for y in y0..=y1 {
        for x in x0..=x1 {
            blend_coverage(buf, w, h, x, y, color, 1.0);
        }
    }
}

fn rounded_rect_radius(a: PixelPoint, b: PixelPoint, stroke: u32) -> u32 {
    let width = a.x.abs_diff(b.x);
    let height = a.y.abs_diff(b.y);
    let half_short_edge = width.min(height) / 2;
    stroke.saturating_mul(4).clamp(4, 48).min(half_short_edge)
}

#[derive(Debug, Clone, Copy)]
struct RoundedRectGeometry {
    a: PixelPoint,
    b: PixelPoint,
    radius: u32,
}

/// 圆角矩形填充：距离场边缘抗锯齿，随后由描边恢复清晰边界。
fn draw_rounded_rect_fill(
    buf: &mut [u8],
    w: i32,
    h: i32,
    geometry: RoundedRectGeometry,
    color: [u8; 4],
) {
    if w <= 0 || h <= 0 {
        return;
    }
    let x0 = geometry.a.x.min(geometry.b.x) as f64;
    let y0 = geometry.a.y.min(geometry.b.y) as f64;
    let x1 = geometry.a.x.max(geometry.b.x) as f64;
    let y1 = geometry.a.y.max(geometry.b.y) as f64;
    let half_width = (x1 - x0) * 0.5;
    let half_height = (y1 - y0) * 0.5;
    if half_width < 1.0 || half_height < 1.0 {
        return;
    }

    let radius = f64::from(geometry.radius).min(half_width).min(half_height);
    if radius < 0.5 {
        draw_rect_fill(buf, w, h, geometry.a, geometry.b, color);
        return;
    }

    let center_x = (x0 + x1) * 0.5;
    let center_y = (y0 + y1) * 0.5;
    let inner_width = (half_width - radius).max(0.0);
    let inner_height = (half_height - radius).max(0.0);
    let aa = 1.0;
    let min_x = (x0.floor() as i32 - 1).max(0);
    let max_x = (x1.ceil() as i32 + 1).min(w - 1);
    let min_y = (y0.floor() as i32 - 1).max(0);
    let max_y = (y1.ceil() as i32 + 1).min(h - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            let qx = (px - center_x).abs() - inner_width;
            let qy = (py - center_y).abs() - inner_height;
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let inside = qx.max(qy).min(0.0);
            let signed_distance = outside + inside - radius;
            let coverage = ((aa * 0.5 - signed_distance) / aa).clamp(0.0, 1.0);
            if coverage > 0.0 {
                blend_coverage(buf, w, h, x, y, color, coverage);
            }
        }
    }
}

/// 抗锯齿圆角矩形描边：圆角半径由已提交对象冻结，并钳制到短边的一半。
fn draw_rounded_rect_outline(
    buf: &mut [u8],
    w: i32,
    h: i32,
    geometry: RoundedRectGeometry,
    color: [u8; 4],
    stroke: u32,
) {
    let x0 = geometry.a.x.min(geometry.b.x) as f64;
    let y0 = geometry.a.y.min(geometry.b.y) as f64;
    let x1 = geometry.a.x.max(geometry.b.x) as f64;
    let y1 = geometry.a.y.max(geometry.b.y) as f64;
    let half_width = (x1 - x0) * 0.5;
    let half_height = (y1 - y0) * 0.5;
    if half_width < 1.0 || half_height < 1.0 {
        return;
    }

    let radius = f64::from(geometry.radius).min(half_width).min(half_height);
    if radius < 0.5 {
        draw_rect_outline(buf, w, h, geometry.a, geometry.b, color, stroke);
        return;
    }

    let center_x = (x0 + x1) * 0.5;
    let center_y = (y0 + y1) * 0.5;
    let inner_width = (half_width - radius).max(0.0);
    let inner_height = (half_height - radius).max(0.0);
    let half_stroke = (stroke as f64 * 0.5).max(0.75);
    let aa = 1.0;
    let pad = (half_stroke + aa + 1.0).ceil() as i32;
    let min_x = (x0.floor() as i32 - pad).max(0);
    let max_x = (x1.ceil() as i32 + pad).min(w - 1);
    let min_y = (y0.floor() as i32 - pad).max(0);
    let max_y = (y1.ceil() as i32 + pad).min(h - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            let qx = (px - center_x).abs() - inner_width;
            let qy = (py - center_y).abs() - inner_height;
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let inside = qx.max(qy).min(0.0);
            let distance_to_edge = (outside + inside - radius).abs();
            let coverage = ((half_stroke + aa * 0.5 - distance_to_edge) / aa).clamp(0.0, 1.0);
            if coverage > 0.0 {
                blend_coverage(buf, w, h, x, y, color, coverage);
            }
        }
    }
}

fn draw_polyline(
    buf: &mut [u8],
    w: i32,
    h: i32,
    points: &[PixelPoint],
    color: [u8; 4],
    stroke: u32,
) {
    if points.is_empty() {
        return;
    }
    let r = (stroke as f64 * 0.5).max(0.75);
    stamp_disc(buf, w, h, points[0].x as f64, points[0].y as f64, r, color);
    for win in points.windows(2) {
        draw_line(buf, w, h, win[0], win[1], color, stroke);
        stamp_disc(buf, w, h, win[1].x as f64, win[1].y as f64, r, color);
    }
}

fn draw_arrow(
    buf: &mut [u8],
    w: i32,
    h: i32,
    from: PixelPoint,
    to: PixelPoint,
    color: [u8; 4],
    stroke: u32,
) {
    draw_line(buf, w, h, from, to, color, stroke);
    let dx = (to.x - from.x) as f64;
    let dy = (to.y - from.y) as f64;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;
    let head = 14.0_f64.max(stroke as f64 * 3.5);
    let bx = to.x as f64 - ux * head;
    let by = to.y as f64 - uy * head;
    let px = -uy;
    let py = ux;
    let wing = head * 0.48;
    let lx = (bx + px * wing).round() as i32;
    let ly = (by + py * wing).round() as i32;
    let rx = (bx - px * wing).round() as i32;
    let ry = (by - py * wing).round() as i32;
    draw_line(buf, w, h, to, PixelPoint::new(lx, ly), color, stroke);
    draw_line(buf, w, h, to, PixelPoint::new(rx, ry), color, stroke);
    let r = (stroke as f64 * 0.5).max(0.75);
    stamp_disc(buf, w, h, to.x as f64, to.y as f64, r, color);
}

fn sequence_marker_diameter(stroke: u32, value: u32) -> u32 {
    let digits = value.to_string().len() as u32;
    let glyph_scale = stroke.clamp(1, 4);
    let label_width = digits.saturating_mul(4).saturating_mul(glyph_scale);
    label_width
        .saturating_add(stroke.saturating_mul(4))
        .saturating_add(10)
        .clamp(20, 128)
}

fn contrast_color(color: [u8; 4]) -> [u8; 4] {
    let luma = u32::from(color[0]) * 299 + u32::from(color[1]) * 587 + u32::from(color[2]) * 114;
    if luma >= 128_000 {
        [16, 16, 20, 255]
    } else {
        [255, 255, 255, 255]
    }
}

fn draw_sequence_marker(
    buf: &mut [u8],
    w: i32,
    h: i32,
    center: PixelPoint,
    value: u32,
    color: [u8; 4],
    diameter: u32,
) {
    let diameter = diameter.clamp(20, 128);
    let radius = f64::from(diameter) * 0.5;
    let contrast = contrast_color(color);
    stamp_disc(buf, w, h, center.x as f64, center.y as f64, radius, color);
    let half = (diameter / 2) as i32;
    draw_ellipse_outline(
        buf,
        w,
        h,
        PixelPoint::new(center.x - half, center.y - half),
        PixelPoint::new(center.x + half, center.y + half),
        contrast,
        (diameter / 16).max(1),
    );
    draw_sequence_label(buf, w, h, center, value, diameter, contrast);
}

fn draw_sequence_label(
    buf: &mut [u8],
    w: i32,
    h: i32,
    center: PixelPoint,
    value: u32,
    diameter: u32,
    color: [u8; 4],
) {
    const DIGITS: [[u8; 5]; 10] = [
        [0b111, 0b101, 0b101, 0b101, 0b111],
        [0b010, 0b110, 0b010, 0b010, 0b111],
        [0b111, 0b001, 0b111, 0b100, 0b111],
        [0b111, 0b001, 0b111, 0b001, 0b111],
        [0b101, 0b101, 0b111, 0b001, 0b001],
        [0b111, 0b100, 0b111, 0b001, 0b111],
        [0b111, 0b100, 0b111, 0b101, 0b111],
        [0b111, 0b001, 0b010, 0b010, 0b010],
        [0b111, 0b101, 0b111, 0b101, 0b111],
        [0b111, 0b101, 0b111, 0b001, 0b111],
    ];

    let text = value.to_string();
    let digit_count = text.len() as u32;
    let cells = digit_count.saturating_mul(4).saturating_sub(1).max(1);
    let scale = (diameter.saturating_sub(10) / cells).clamp(1, 4) as i32;
    let total_width = cells as i32 * scale;
    let total_height = 5 * scale;
    let start_x = center.x - total_width / 2;
    let start_y = center.y - total_height / 2;

    for (index, digit) in text.bytes().enumerate() {
        let glyph = DIGITS[usize::from(digit - b'0')];
        let glyph_x = start_x + index as i32 * 4 * scale;
        for (row, mask) in glyph.iter().enumerate() {
            for column in 0..3 {
                if mask & (1 << (2 - column)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        blend_coverage(
                            buf,
                            w,
                            h,
                            glyph_x + column * scale + dx,
                            start_y + row as i32 * scale + dy,
                            color,
                            1.0,
                        );
                    }
                }
            }
        }
    }
}

/// 椭圆填充：边界按隐式方程距离近似抗锯齿，内部始终走 alpha-over 合成。
fn draw_ellipse_fill(buf: &mut [u8], w: i32, h: i32, a: PixelPoint, b: PixelPoint, color: [u8; 4]) {
    if w <= 0 || h <= 0 {
        return;
    }
    let x0 = a.x.min(b.x) as f64;
    let y0 = a.y.min(b.y) as f64;
    let x1 = a.x.max(b.x) as f64;
    let y1 = a.y.max(b.y) as f64;
    let cx = (x0 + x1) * 0.5;
    let cy = (y0 + y1) * 0.5;
    let rx = ((x1 - x0) * 0.5).max(1.0);
    let ry = ((y1 - y0) * 0.5).max(1.0);
    let aa = 1.0;
    let min_x = ((cx - rx).floor() as i32 - 1).max(0);
    let max_x = ((cx + rx).ceil() as i32 + 1).min(w - 1);
    let min_y = ((cy - ry).floor() as i32 - 1).max(0);
    let max_y = ((cy + ry).ceil() as i32 + 1).min(h - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            let nx = (px - cx) / rx;
            let ny = (py - cy) / ry;
            let r_norm = (nx * nx + ny * ny).sqrt();
            let signed_distance = if r_norm < 1e-6 {
                -rx.min(ry)
            } else {
                let grad = ((nx / rx).powi(2) + (ny / ry).powi(2)).sqrt().max(1e-6);
                (r_norm - 1.0) / grad
            };
            let coverage = ((aa * 0.5 - signed_distance) / aa).clamp(0.0, 1.0);
            if coverage > 0.0 {
                blend_coverage(buf, w, h, x, y, color, coverage);
            }
        }
    }
}

/// 椭圆轮廓：用隐式方程距离近似做抗锯齿描边。
fn draw_ellipse_outline(
    buf: &mut [u8],
    w: i32,
    h: i32,
    a: PixelPoint,
    b: PixelPoint,
    color: [u8; 4],
    stroke: u32,
) {
    let x0 = a.x.min(b.x) as f64;
    let y0 = a.y.min(b.y) as f64;
    let x1 = a.x.max(b.x) as f64;
    let y1 = a.y.max(b.y) as f64;
    let cx = (x0 + x1) * 0.5;
    let cy = (y0 + y1) * 0.5;
    let rx = ((x1 - x0) * 0.5).max(1.0);
    let ry = ((y1 - y0) * 0.5).max(1.0);
    let half = (stroke as f64 * 0.5).max(0.75);
    let aa = 1.0;
    let pad = (half + aa + 2.0).ceil() as i32;

    let min_x = ((cx - rx).floor() as i32 - pad).max(0);
    let max_x = ((cx + rx).ceil() as i32 + pad).min(w - 1);
    let min_y = ((cy - ry).floor() as i32 - pad).max(0);
    let max_y = ((cy + ry).ceil() as i32 + pad).min(h - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            // 归一化半径 r_norm；边界处 = 1
            let nx = (px - cx) / rx;
            let ny = (py - cy) / ry;
            let r_norm = (nx * nx + ny * ny).sqrt();
            if r_norm < 1e-6 {
                continue;
            }
            // 近似到椭圆边界的欧氏距离（梯度归一化）
            let grad = ((nx / rx).powi(2) + (ny / ry).powi(2)).sqrt().max(1e-6);
            let dist = ((r_norm - 1.0) / grad).abs();
            let cov = ((half + aa * 0.5 - dist) / aa).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_coverage(buf, w, h, x, y, color, cov);
            }
        }
    }
}

/// 马赛克：将选区内像素按 block 平均后回填。
fn draw_mosaic(
    buf: &mut [u8],
    src: &[u8],
    w: i32,
    h: i32,
    a: PixelPoint,
    b: PixelPoint,
    block: u32,
) {
    let x0 = a.x.min(b.x).max(0);
    let y0 = a.y.min(b.y).max(0);
    let x1 = a.x.max(b.x).min(w - 1);
    let y1 = a.y.max(b.y).min(h - 1);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let bs = block.max(2) as i32;
    let mut by = y0;
    while by <= y1 {
        let mut bx = x0;
        while bx <= x1 {
            let ex = (bx + bs - 1).min(x1);
            let ey = (by + bs - 1).min(y1);
            let mut sum = [0u64; 4];
            let mut n = 0u64;
            for y in by..=ey {
                for x in bx..=ex {
                    let i = ((y * w + x) * 4) as usize;
                    if i + 3 < src.len() {
                        sum[0] += src[i] as u64;
                        sum[1] += src[i + 1] as u64;
                        sum[2] += src[i + 2] as u64;
                        sum[3] += src[i + 3] as u64;
                        n += 1;
                    }
                }
            }
            if let Some(sample_count) = NonZeroU64::new(n) {
                let avg = [
                    (sum[0] / sample_count) as u8,
                    (sum[1] / sample_count) as u8,
                    (sum[2] / sample_count) as u8,
                    (sum[3] / sample_count) as u8,
                ];
                for y in by..=ey {
                    for x in bx..=ex {
                        let i = ((y * w + x) * 4) as usize;
                        if i + 3 < buf.len() {
                            buf[i] = avg[0];
                            buf[i + 1] = avg[1];
                            buf[i + 2] = avg[2];
                            buf[i + 3] = avg[3];
                        }
                    }
                }
            }
            bx += bs;
        }
        by += bs;
    }
}

/// 候选系统字体路径（优先 CJK TTF）。
fn font_candidates() -> &'static [&'static str] {
    &[
        "/usr/share/fonts/lxgw-wenkai-fonts/LXGWWenKai-Regular.ttf",
        "/usr/share/fonts/lxgw-wenkai-fonts/LXGWWenKai-Medium.ttf",
        "/usr/share/fonts/google-droid-sans-fonts/DroidSansFallbackFull.ttf",
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
    ]
}

struct FontCache {
    font: fontdue::Font,
}

fn load_font() -> Option<&'static FontCache> {
    static FONT: OnceLock<Option<FontCache>> = OnceLock::new();
    FONT.get_or_init(|| {
        for path in font_candidates() {
            if let Ok(bytes) = std::fs::read(path) {
                // TTC 可能失败；仅接受可解析字体
                if let Ok(font) =
                    fontdue::Font::from_bytes(bytes.as_slice(), fontdue::FontSettings::default())
                {
                    return Some(FontCache { font });
                }
            }
        }
        None
    })
    .as_ref()
}

fn draw_text(
    buf: &mut [u8],
    w: i32,
    h: i32,
    origin: PixelPoint,
    content: &str,
    color: [u8; 4],
    size: f32,
) {
    let Some(cache) = load_font() else {
        // 无字体：画占位横线
        draw_line(
            buf,
            w,
            h,
            origin,
            PixelPoint::new(
                origin.x + (content.chars().count() as i32 * 8).max(8),
                origin.y,
            ),
            color,
            2,
        );
        return;
    };
    let size = size.clamp(8.0, 96.0);
    let mut pen_x = origin.x as f32;
    let pen_y = origin.y as f32;
    for ch in content.chars() {
        if ch == '\n' {
            continue;
        }
        let (metrics, bitmap) = cache.font.rasterize(ch, size);
        // fontdue：位图原点相对基线左下系；ymin 常为负（上移）
        let gx = (pen_x + metrics.xmin as f32).round() as i32;
        let gy = (pen_y + metrics.ymin as f32).round() as i32;
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + col] as f64 / 255.0;
                if alpha > 0.01 {
                    blend_coverage(buf, w, h, gx + col as i32, gy + row as i32, color, alpha);
                }
            }
        }
        pen_x += metrics.advance_width;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{PixelRect, PixelSize};
    use crate::image::{CaptureMetadata, DisplayId};

    fn solid(w: u32, h: u32) -> CaptureImage {
        CaptureImage::new(
            ImageId::new(),
            RgbaBuffer::solid(PixelSize::new(w, h), [255, 255, 255, 255]),
            PixelRect::new(0, 0, w, h),
            CaptureMetadata::new(DisplayId::new("d0"), 1.0, 0),
        )
        .unwrap()
    }

    #[test]
    fn rect_session_commit() {
        let mut s = AnnotateSession::new(100, 80);
        s.begin(PixelPoint::new(10, 10));
        s.drag(PixelPoint::new(40, 30));
        s.commit();
        assert_eq!(s.doc.len(), 1);
    }

    #[test]
    fn rounded_rect_freezes_its_radius_and_preview_matches_the_committed_render() {
        let source = solid(100, 80);
        let mut session = AnnotateSession::new(100, 80);
        session.tool = AnnotateTool::RoundedRect;
        session.stroke = 5;
        session.begin(PixelPoint::new(10, 10));
        session.drag(PixelPoint::new(90, 70));

        let preview = render_preview_rgba(&source, &session);
        assert!(session.doc.is_empty());
        session.commit();
        assert!(matches!(
            session.doc.items(),
            [Annotation::RoundedRect {
                radius: 20,
                stroke: 5,
                ..
            }]
        ));

        session.stroke = 1;
        let baked = bake_annotations(&source, &session.doc);
        assert_eq!(preview, baked.pixels.bytes);
        assert_ne!(baked.pixels.bytes, source.pixels.bytes);
    }

    #[test]
    fn rounded_rect_rejects_short_edges_and_clamps_an_oversized_radius_safely() {
        let mut session = AnnotateSession::new(24, 20);
        session.tool = AnnotateTool::RoundedRect;
        let initial = session.doc.revision();
        session.begin(PixelPoint::new(5, 3));
        session.drag(PixelPoint::new(6, 18));
        session.commit();
        assert!(session.doc.is_empty());
        assert_eq!(session.doc.revision(), initial);

        let source = solid(24, 20);
        let mut doc = AnnotationDoc::new();
        doc.push(Annotation::RoundedRect {
            a: PixelPoint::new(2, 2),
            b: PixelPoint::new(21, 17),
            color: [30, 120, 220, 255],
            stroke: 3,
            radius: u32::MAX,
            fill: None,
        });
        let baked = bake_annotations(&source, &doc);
        assert_eq!(baked.pixels.bytes.len(), source.pixels.bytes.len());
        assert_ne!(baked.pixels.bytes, source.pixels.bytes);
        assert_eq!(
            rounded_rect_radius(PixelPoint::new(2, 2), PixelPoint::new(21, 17), 99),
            7
        );
    }

    #[test]
    fn line_session_requires_a_real_drag_and_commits_once() {
        let mut session = AnnotateSession::new(100, 80);
        session.tool = AnnotateTool::Line;
        let initial = session.doc.revision();

        session.begin(PixelPoint::new(10, 10));
        session.drag(PixelPoint::new(11, 11));
        session.commit();
        assert!(session.doc.is_empty());
        assert_eq!(session.doc.revision(), initial);

        session.begin(PixelPoint::new(10, 10));
        session.drag(PixelPoint::new(40, 30));
        session.commit();
        assert_eq!(session.doc.len(), 1);
        assert_eq!(session.doc.revision(), initial.advance());
        assert!(matches!(
            session.doc.items(),
            [Annotation::Line {
                from,
                to,
                color: DEFAULT_STROKE,
                stroke: DEFAULT_WIDTH,
            }] if *from == PixelPoint::new(10, 10) && *to == PixelPoint::new(40, 30)
        ));
    }

    #[test]
    fn number_markers_are_monotonic_and_undo_does_not_rewrite_values() {
        let mut session = AnnotateSession::new(100, 80);
        session.tool = AnnotateTool::Number;
        assert_eq!(session.set_sequence_start(12), 12);

        session.begin(PixelPoint::new(10, 10));
        session.begin(PixelPoint::new(30, 10));
        assert_eq!(session.next_sequence_number(), Some(14));
        assert!(matches!(
            session.doc.items(),
            [
                Annotation::Number { value: 12, .. },
                Annotation::Number { value: 13, .. },
            ]
        ));

        assert!(session.doc.undo().is_some());
        assert_eq!(session.next_sequence_number(), Some(14));
        assert!(session.doc.redo().is_some());
        assert_eq!(session.next_sequence_number(), Some(14));

        session.begin(PixelPoint::new(50, 10));
        assert!(matches!(
            session.doc.items(),
            [
                Annotation::Number { value: 12, .. },
                Annotation::Number { value: 13, .. },
                Annotation::Number { value: 14, .. },
            ]
        ));
    }

    #[test]
    fn number_marker_stops_after_the_configured_upper_bound() {
        let mut session = AnnotateSession::new(100, 80);
        session.tool = AnnotateTool::Number;
        assert_eq!(session.set_sequence_start(u32::MAX), MAX_SEQUENCE_NUMBER);

        session.begin(PixelPoint::new(10, 10));
        assert_eq!(session.next_sequence_number(), None);
        assert_eq!(session.doc.len(), 1);
        session.begin(PixelPoint::new(30, 10));
        assert_eq!(session.doc.len(), 1);
        assert!(matches!(
            session.doc.items(),
            [Annotation::Number {
                value: MAX_SEQUENCE_NUMBER,
                ..
            }]
        ));
    }

    #[test]
    fn line_and_number_render_without_mutating_the_source() {
        let source = solid(96, 72);
        let mut doc = AnnotationDoc::new();
        doc.push(Annotation::Line {
            from: PixelPoint::new(4, 4),
            to: PixelPoint::new(70, 40),
            color: [220, 40, 40, 255],
            stroke: 3,
        });
        doc.push(Annotation::Number {
            center: PixelPoint::new(60, 52),
            value: 12,
            color: [40, 120, 220, 255],
            diameter: 30,
        });

        let baked = bake_annotations(&source, &doc);
        assert_ne!(baked.pixels.bytes, source.pixels.bytes);
        assert_eq!(source.pixels.bytes, solid(96, 72).pixels.bytes);
        assert_eq!(baked.pixels.size, source.pixels.size);
    }

    #[test]
    fn bake_changes_pixels() {
        let src = solid(20, 20);
        let mut doc = AnnotationDoc::new();
        doc.push(Annotation::Rect {
            a: PixelPoint::new(2, 2),
            b: PixelPoint::new(10, 10),
            color: [255, 0, 0, 255],
            stroke: 2,
            fill: None,
        });
        let out = bake_annotations(&src, &doc);
        assert_ne!(out.pixels.bytes, src.pixels.bytes);
        assert_eq!(out.pixels.size, src.pixels.size);
    }

    #[test]
    fn undo_pops() {
        let mut s = AnnotateSession::new(50, 50);
        s.tool = AnnotateTool::Pen;
        s.begin(PixelPoint::new(1, 1));
        s.drag(PixelPoint::new(5, 5));
        s.commit();
        assert_eq!(s.doc.len(), 1);
        s.doc.undo();
        assert!(s.doc.is_empty());
    }

    #[test]
    fn ellipse_and_mosaic_commit() {
        let mut s = AnnotateSession::new(80, 60);
        s.tool = AnnotateTool::Ellipse;
        s.begin(PixelPoint::new(5, 5));
        s.drag(PixelPoint::new(40, 30));
        s.commit();
        s.tool = AnnotateTool::Mosaic;
        s.begin(PixelPoint::new(10, 10));
        s.drag(PixelPoint::new(30, 25));
        s.commit();
        assert_eq!(s.doc.len(), 2);
        let src = solid(80, 60);
        let out = bake_annotations(&src, &s.doc);
        assert_ne!(out.pixels.bytes, src.pixels.bytes);
    }

    #[test]
    fn shape_fill_toggle_is_non_transactional_and_freezes_preview_style() {
        let source = solid(32, 24);
        let mut session = AnnotateSession::new(32, 24);
        session.set_color([24, 120, 220, 255]);
        let initial_revision = session.doc.revision();

        assert!(!session.shape_fill_enabled());
        assert!(session.toggle_shape_fill());
        assert_eq!(session.doc.revision(), initial_revision);

        session.begin(PixelPoint::new(4, 4));
        session.drag(PixelPoint::new(24, 18));
        let preview = render_preview_rgba(&source, &session);
        session.commit();
        assert!(matches!(
            session.doc.items(),
            [Annotation::Rect {
                color: [24, 120, 220, 255],
                fill: Some([24, 120, 220, SHAPE_FILL_ALPHA]),
                ..
            }]
        ));

        session.set_color([255, 64, 64, 255]);
        assert!(!session.toggle_shape_fill());
        let baked = bake_annotations(&source, &session.doc);
        assert_eq!(preview, baked.pixels.bytes);

        let inside = rgba_at(&baked.pixels.bytes, 32, 12, 12);
        assert_eq!(inside, [168, 204, 241, 255]);
        let edge = rgba_at(&baked.pixels.bytes, 32, 12, 4);
        assert_eq!(edge, [24, 120, 220, 255]);
    }

    #[test]
    fn all_closed_shapes_fill_their_interior_without_affecting_stroke_only_shapes() {
        let source = solid(56, 32);
        let fill = Some([40, 120, 220, SHAPE_FILL_ALPHA]);
        let mut doc = AnnotationDoc::new();
        doc.push(Annotation::Rect {
            a: PixelPoint::new(2, 4),
            b: PixelPoint::new(14, 20),
            color: [40, 120, 220, 255],
            stroke: 2,
            fill,
        });
        doc.push(Annotation::RoundedRect {
            a: PixelPoint::new(20, 4),
            b: PixelPoint::new(34, 20),
            color: [40, 120, 220, 255],
            stroke: 2,
            radius: 4,
            fill,
        });
        doc.push(Annotation::Ellipse {
            a: PixelPoint::new(40, 4),
            b: PixelPoint::new(54, 20),
            color: [40, 120, 220, 255],
            stroke: 2,
            fill,
        });
        doc.push(Annotation::Rect {
            a: PixelPoint::new(2, 24),
            b: PixelPoint::new(14, 30),
            color: [40, 120, 220, 255],
            stroke: 2,
            fill: None,
        });

        let baked = bake_annotations(&source, &doc);
        let expected_fill = [174, 204, 241, 255];
        for (x, y) in [(8, 12), (27, 12), (47, 12)] {
            assert_eq!(rgba_at(&baked.pixels.bytes, 56, x, y), expected_fill);
        }
        assert_eq!(
            rgba_at(&baked.pixels.bytes, 56, 8, 27),
            [255, 255, 255, 255]
        );
        assert_eq!(rgba_at(&baked.pixels.bytes, 56, 8, 24), [40, 120, 220, 255]);
    }

    fn rgba_at(bytes: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
        let index = (y * width + x) * 4;
        [
            bytes[index],
            bytes[index + 1],
            bytes[index + 2],
            bytes[index + 3],
        ]
    }

    #[test]
    fn color_and_stroke_cycle() {
        let mut s = AnnotateSession::new(10, 10);
        let c0 = s.color;
        s.cycle_color();
        assert_ne!(s.color, c0);
        let w0 = s.stroke;
        s.stroke_up();
        assert!(s.stroke > w0);
        s.stroke_down();
        assert_eq!(s.stroke, w0);
    }

    #[test]
    fn pixel_sampling_and_hex_format_are_stable_without_a_document_mutation() {
        let mut image = solid(3, 2);
        image.pixels.bytes[4..8].copy_from_slice(&[0x12, 0xA0, 0xFE, 0x80]);
        let color = sample_rgba_at(&image, PixelPoint::new(1, 0));
        assert_eq!(color, Some([0x12, 0xA0, 0xFE, 0x80]));
        assert_eq!(color.map(color_to_hex).as_deref(), Some("#12A0FE"));
        assert_eq!(sample_rgba_at(&image, PixelPoint::new(-1, 0)), None);
        assert_eq!(sample_rgba_at(&image, PixelPoint::new(3, 0)), None);

        let mut session = AnnotateSession::new(3, 2);
        let revision = session.doc.revision();
        session.tool = AnnotateTool::ColorPicker;
        session.begin(PixelPoint::new(1, 0));
        session.set_color(color.expect("sampled color"));
        assert!(session.draft.is_none());
        assert_eq!(session.doc.revision(), revision);
        assert_eq!(session.color, [0x12, 0xA0, 0xFE, 0x80]);
    }

    #[test]
    fn text_session_commit() {
        let mut s = AnnotateSession::new(100, 40);
        s.tool = AnnotateTool::Text;
        s.begin(PixelPoint::new(8, 24));
        s.text_push("hi");
        assert!(s.is_text_editing());
        s.commit();
        assert_eq!(s.doc.len(), 1);
        match &s.doc.items()[0] {
            Annotation::Text { content, .. } => assert_eq!(content, "hi"),
            _ => panic!("expected text"),
        }
        let src = solid(100, 40);
        let out = bake_annotations(&src, &s.doc);
        // 有系统字体时应改变像素；无字体时也会画占位线
        assert_ne!(out.pixels.bytes, src.pixels.bytes);
    }

    #[test]
    fn revision_advances_only_for_effective_document_mutations() {
        let mut doc = AnnotationDoc::new();
        assert_eq!(doc.revision().raw(), 1);
        assert_eq!(doc.undo(), None);
        assert_eq!(doc.revision().raw(), 1);

        doc.push(Annotation::Rect {
            a: PixelPoint::new(1, 1),
            b: PixelPoint::new(6, 6),
            color: DEFAULT_STROKE,
            stroke: DEFAULT_WIDTH,
            fill: None,
        });
        assert_eq!(doc.revision().raw(), 2);
        assert!(doc.undo().is_some());
        assert_eq!(doc.revision().raw(), 3);
        assert_eq!(doc.undo(), None);
        assert_eq!(doc.revision().raw(), 3);
    }

    #[test]
    fn revision_is_non_zero_and_never_wraps() {
        assert_eq!(AnnotationRevision::from_raw(0), None);
        let maximum = AnnotationRevision::from_raw(u64::MAX).expect("maximum revision");
        assert_eq!(maximum.advance(), maximum);
    }

    #[test]
    fn undo_and_redo_restore_annotations_in_lifo_order() {
        let first = Annotation::Rect {
            a: PixelPoint::new(1, 1),
            b: PixelPoint::new(4, 4),
            color: DEFAULT_STROKE,
            stroke: DEFAULT_WIDTH,
            fill: None,
        };
        let second = Annotation::Rect {
            a: PixelPoint::new(5, 5),
            b: PixelPoint::new(9, 9),
            color: DEFAULT_STROKE,
            stroke: DEFAULT_WIDTH,
            fill: None,
        };
        let mut doc = AnnotationDoc::new();
        doc.push(first.clone());
        doc.push(second.clone());
        assert_eq!(doc.revision().raw(), 3);

        assert_eq!(doc.undo(), Some(second.clone()));
        assert_eq!(doc.undo(), Some(first.clone()));
        assert!(doc.items().is_empty());
        assert!(doc.can_redo());
        assert_eq!(doc.revision().raw(), 5);

        assert_eq!(doc.redo(), Some(first.clone()));
        assert_eq!(doc.items(), std::slice::from_ref(&first));
        assert!(doc.can_redo());
        assert_eq!(doc.redo(), Some(second.clone()));
        assert_eq!(doc.items(), [first, second]);
        assert!(!doc.can_redo());
        assert_eq!(doc.revision().raw(), 7);
        assert_eq!(doc.redo(), None);
        assert_eq!(doc.revision().raw(), 7);
    }

    #[test]
    fn new_annotation_clears_redo_branch() {
        let mut doc = AnnotationDoc::new();
        let original = Annotation::Rect {
            a: PixelPoint::new(1, 1),
            b: PixelPoint::new(4, 4),
            color: DEFAULT_STROKE,
            stroke: DEFAULT_WIDTH,
            fill: None,
        };
        doc.push(original);
        assert!(doc.undo().is_some());
        assert!(doc.can_redo());

        doc.push(Annotation::Rect {
            a: PixelPoint::new(6, 6),
            b: PixelPoint::new(9, 9),
            color: DEFAULT_STROKE,
            stroke: DEFAULT_WIDTH,
            fill: None,
        });
        assert!(!doc.can_redo());
        let revision = doc.revision();
        assert_eq!(doc.redo(), None);
        assert_eq!(doc.revision(), revision);
    }

    #[test]
    fn invalid_or_cancelled_drafts_do_not_advance_revision() {
        let mut session = AnnotateSession::new(40, 30);
        let initial = session.doc.revision();

        session.begin(PixelPoint::new(3, 3));
        session.commit();
        assert_eq!(session.doc.revision(), initial);

        session.tool = AnnotateTool::Text;
        session.begin(PixelPoint::new(3, 3));
        session.text_push(" \n\t ");
        session.commit();
        assert_eq!(session.doc.revision(), initial);

        session.begin(PixelPoint::new(4, 4));
        session.cancel_draft();
        assert_eq!(session.doc.revision(), initial);
    }
}
