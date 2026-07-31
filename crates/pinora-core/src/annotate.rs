//! 基础标注：矩形、箭头、自由画笔；图像坐标；栅格化到 RGBA。

use crate::geometry::PixelPoint;
use crate::image::{CaptureImage, RgbaBuffer};
use crate::ids::ImageId;

/// 标注工具。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AnnotateTool {
    #[default]
    Rect,
    Arrow,
    Pen,
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
}

/// 标注文档（图像坐标系）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnnotationDoc {
    pub items: Vec<Annotation>,
}

impl AnnotationDoc {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, item: Annotation) {
        self.items.push(item);
    }

    pub fn undo(&mut self) -> Option<Annotation> {
        self.items.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// 默认描边色（亮红）。
pub const DEFAULT_STROKE: [u8; 4] = [255, 64, 64, 255];
pub const DEFAULT_WIDTH: u32 = 3;

/// 进行中的拖拽草稿。
#[derive(Debug, Clone, PartialEq)]
pub enum DraftShape {
    Rect { a: PixelPoint, b: PixelPoint },
    Arrow { from: PixelPoint, to: PixelPoint },
    Pen { points: Vec<PixelPoint> },
}

/// 标注编辑会话（纯逻辑）。
#[derive(Debug, Clone)]
pub struct AnnotateSession {
    pub tool: AnnotateTool,
    pub doc: AnnotationDoc,
    pub draft: Option<DraftShape>,
    pub color: [u8; 4],
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

    pub fn begin(&mut self, p: PixelPoint) {
        let p = self.clamp_point(p);
        self.draft = Some(match self.tool {
            AnnotateTool::Rect => DraftShape::Rect { a: p, b: p },
            AnnotateTool::Arrow => DraftShape::Arrow { from: p, to: p },
            AnnotateTool::Pen => DraftShape::Pen { points: vec![p] },
        });
    }

    pub fn drag(&mut self, p: PixelPoint) {
        let p = self.clamp_point(p);
        match &mut self.draft {
            Some(DraftShape::Rect { b, .. }) => *b = p,
            Some(DraftShape::Arrow { to, .. }) => *to = p,
            Some(DraftShape::Pen { points }) => {
                // 插值补点，折线更密、AA 更顺滑
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
            None => {}
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
    for item in &doc.items {
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
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 4],
    stroke: u32,
) {
    let x0 = x0 as f64;
    let y0 = y0 as f64;
    let x1 = x1 as f64;
    let y1 = y1 as f64;
    let radius = (stroke as f64 * 0.5).max(0.75);
    let aa = 1.0; // 软边宽度（像素）
    let pad = (radius + aa + 1.0).ceil() as i32;

    let min_x = x0.min(x1).floor() as i32 - pad;
    let max_x = x0.max(x1).ceil() as i32 + pad;
    let min_y = y0.min(y1).floor() as i32 - pad;
    let max_y = y0.max(y1).ceil() as i32 + pad;

    let min_x = min_x.max(0);
    let min_y = min_y.max(0);
    let max_x = max_x.min(w - 1);
    let max_y = max_y.min(h - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            // 像素中心采样
            let d = dist_point_segment(x as f64 + 0.5, y as f64 + 0.5, x0, y0, x1, y1);
            // 覆盖率：半径内为 1，外缘 1px 线性衰减
            let cov = ((radius + aa * 0.5 - d) / aa).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_coverage(buf, w, h, x, y, color, cov);
            }
        }
    }
}

/// 抗锯齿圆点（画笔端点 / 补点）。
fn stamp_disc(
    buf: &mut [u8],
    w: i32,
    h: i32,
    cx: f64,
    cy: f64,
    radius: f64,
    color: [u8; 4],
) {
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
    // 四边独立 AA 线；角点自然重叠混合
    draw_line(buf, w, h, x0, y0, x1, y0, color, stroke);
    draw_line(buf, w, h, x1, y0, x1, y1, color, stroke);
    draw_line(buf, w, h, x1, y1, x0, y1, color, stroke);
    draw_line(buf, w, h, x0, y1, x0, y0, color, stroke);
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
    // 端点圆头，避免折线关节缺口
    stamp_disc(buf, w, h, points[0].x as f64, points[0].y as f64, r, color);
    for win in points.windows(2) {
        draw_line(
            buf,
            w,
            h,
            win[0].x,
            win[0].y,
            win[1].x,
            win[1].y,
            color,
            stroke,
        );
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
    draw_line(buf, w, h, from.x, from.y, to.x, to.y, color, stroke);
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
    draw_line(buf, w, h, to.x, to.y, lx, ly, color, stroke);
    draw_line(buf, w, h, to.x, to.y, rx, ry, color, stroke);
    // 箭头尖端更圆润
    let r = (stroke as f64 * 0.5).max(0.75);
    stamp_disc(buf, w, h, to.x as f64, to.y as f64, r, color);
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
}
