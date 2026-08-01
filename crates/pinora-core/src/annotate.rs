//! 标注：矩形/箭头/画笔/椭圆/马赛克/文本；颜色与线宽；栅格化到 RGBA。

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
    Arrow,
    Pen,
    Ellipse,
    Mosaic,
    Text,
}

/// 单条标注（已提交）。
#[derive(Debug, Clone, PartialEq)]
pub enum Annotation {
    Rect {
        a: PixelPoint,
        b: PixelPoint,
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

    pub fn stroke_up(&mut self) {
        self.stroke = (self.stroke + 1).min(MAX_STROKE);
    }

    pub fn stroke_down(&mut self) {
        self.stroke = self.stroke.saturating_sub(1).max(MIN_STROKE);
    }

    /// 是否正在编辑文本草稿（键入中）。
    pub fn is_text_editing(&self) -> bool {
        matches!(self.draft, Some(DraftShape::Text { .. }))
    }

    pub fn begin(&mut self, p: PixelPoint) {
        let p = self.clamp_point(p);
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
            AnnotateTool::Arrow => DraftShape::Arrow { from: p, to: p },
            AnnotateTool::Pen => DraftShape::Pen { points: vec![p] },
            AnnotateTool::Ellipse => DraftShape::Ellipse { a: p, b: p },
            AnnotateTool::Mosaic => DraftShape::Mosaic { a: p, b: p },
            AnnotateTool::Text => unreachable!(),
        });
    }

    pub fn drag(&mut self, p: PixelPoint) {
        let p = self.clamp_point(p);
        match &mut self.draft {
            Some(DraftShape::Rect { b, .. }) => *b = p,
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
            } => draw_rect_outline(&mut bytes, w, h, *a, *b, *color, *stroke),
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
            } => draw_ellipse_outline(&mut bytes, w, h, *a, *b, *color, *stroke),
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
        match draft {
            DraftShape::Rect { a, b } => doc.push(Annotation::Rect {
                a: *a,
                b: *b,
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
    fn bake_changes_pixels() {
        let src = solid(20, 20);
        let mut doc = AnnotationDoc::new();
        doc.push(Annotation::Rect {
            a: PixelPoint::new(2, 2),
            b: PixelPoint::new(10, 10),
            color: [255, 0, 0, 255],
            stroke: 2,
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
        };
        let second = Annotation::Rect {
            a: PixelPoint::new(5, 5),
            b: PixelPoint::new(9, 9),
            color: DEFAULT_STROKE,
            stroke: DEFAULT_WIDTH,
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
        };
        doc.push(original);
        assert!(doc.undo().is_some());
        assert!(doc.can_redo());

        doc.push(Annotation::Rect {
            a: PixelPoint::new(6, 6),
            b: PixelPoint::new(9, 9),
            color: DEFAULT_STROKE,
            stroke: DEFAULT_WIDTH,
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
