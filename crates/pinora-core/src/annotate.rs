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
                if points.last().copied() != Some(p) {
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

fn put_pixel(buf: &mut [u8], w: i32, h: i32, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let i = ((y * w + x) * 4) as usize;
    if i + 3 >= buf.len() {
        return;
    }
    // 简单 alpha over
    let a = color[3] as u32;
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

fn brush(
    buf: &mut [u8],
    w: i32,
    h: i32,
    x: i32,
    y: i32,
    color: [u8; 4],
    stroke: u32,
) {
    let r = (stroke as i32).max(1) / 2;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r + r {
                put_pixel(buf, w, h, x + dx, y + dy, color);
            }
        }
    }
}

fn draw_line(
    buf: &mut [u8],
    w: i32,
    h: i32,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 4],
    stroke: u32,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        brush(buf, w, h, x0, y0, color, stroke);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
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
    let head = 12.0_f64.max(stroke as f64 * 3.0);
    let bx = to.x as f64 - ux * head;
    let by = to.y as f64 - uy * head;
    let px = -uy;
    let py = ux;
    let wing = head * 0.45;
    let l = PixelPoint::new(
        (bx + px * wing).round() as i32,
        (by + py * wing).round() as i32,
    );
    let r = PixelPoint::new(
        (bx - px * wing).round() as i32,
        (by - py * wing).round() as i32,
    );
    draw_line(buf, w, h, to.x, to.y, l.x, l.y, color, stroke);
    draw_line(buf, w, h, to.x, to.y, r.x, r.y, color, stroke);
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
