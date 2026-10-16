//! Draws a [`Document`] with tiny-skia. The editor's live view and the
//! exported image both come from here, so what you see is what you get.

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};
use image::RgbaImage;
use tiny_skia::{
    BlendMode, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PremultipliedColorU8,
    Stroke, Transform,
};

use super::{arrow_head_size, Color, Document, Point, Shape, ShapeKind};

/// Strength of the blur tool, in pixels.
const BLUR_SIGMA: f32 = 12.0;
/// Block size of the pixelate tool, in pixels.
const PIXEL_BLOCK: u32 = 12;

pub struct Renderer {
    fonts: FontSystem,
    glyphs: SwashCache,
    /// The document as last rendered, keyed by its revision.
    committed: Option<(u64, Pixmap)>,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    /// Loads the system fonts, which takes a moment: create one and keep it.
    pub fn new() -> Self {
        Self {
            fonts: FontSystem::new(),
            glyphs: SwashCache::new(),
            committed: None,
        }
    }

    /// Width and height of `text` at `font_size`, in pixels.
    pub fn measure_text(&mut self, text: &str, font_size: f32) -> (f32, f32) {
        let buffer = self.layout(text, font_size);
        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height = run.line_top + run.line_height;
        }
        (width.ceil(), height.ceil())
    }

    /// The whole capture with its annotations, plus `preview` (a shape being
    /// drawn) on top. Not cropped: the editor shows the crop as an overlay.
    pub fn render(&mut self, doc: &Document, preview: Option<&Shape>) -> Pixmap {
        let stale = self
            .committed
            .as_ref()
            .is_none_or(|(revision, _)| *revision != doc.revision());
        if stale {
            let mut pixmap = to_pixmap(doc.base());
            for shape in doc.shapes() {
                self.draw(&mut pixmap, shape);
            }
            self.committed = Some((doc.revision(), pixmap));
        }
        let mut pixmap = self.committed.as_ref().expect("rendered above").1.clone();
        if let Some(shape) = preview {
            self.draw(&mut pixmap, shape);
        }
        pixmap
    }

    /// The final image: annotations flattened and the crop applied.
    pub fn export(&mut self, doc: &Document) -> RgbaImage {
        let pixmap = self.render(doc, None);
        let image = to_image(&pixmap);
        match doc.crop() {
            Some(c) if c.width >= 1.0 && c.height >= 1.0 => image::imageops::crop_imm(
                &image,
                c.x.round() as u32,
                c.y.round() as u32,
                c.width.round() as u32,
                c.height.round() as u32,
            )
            .to_image(),
            _ => image,
        }
    }

    fn draw(&mut self, pixmap: &mut Pixmap, shape: &Shape) {
        let paint = paint(shape.color);
        let stroke = Stroke {
            width: shape.width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        let id = Transform::identity();
        let rect = shape.rect();
        let sk_rect = tiny_skia::Rect::from_xywh(
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
        );

        match &shape.kind {
            ShapeKind::Rect => {
                if let Some(path) = sk_rect.map(PathBuilder::from_rect) {
                    let stroke = Stroke {
                        line_join: LineJoin::Miter,
                        ..stroke
                    };
                    pixmap.stroke_path(&path, &paint, &stroke, id, None);
                }
            }
            ShapeKind::Ellipse => {
                if let Some(path) = sk_rect.and_then(PathBuilder::from_oval) {
                    pixmap.stroke_path(&path, &paint, &stroke, id, None);
                }
            }
            ShapeKind::Line => {
                if let Some(path) = polyline(&[shape.start, shape.end]) {
                    pixmap.stroke_path(&path, &paint, &stroke, id, None);
                }
            }
            ShapeKind::Arrow => draw_arrow(pixmap, shape, &paint, &stroke),
            ShapeKind::Freehand(points) => {
                if points.len() == 1 {
                    let p = points[0];
                    if let Some(dot) = PathBuilder::from_circle(p.x, p.y, shape.width / 2.0) {
                        pixmap.fill_path(&dot, &paint, FillRule::Winding, id, None);
                    }
                } else if let Some(path) = polyline(points) {
                    pixmap.stroke_path(&path, &paint, &stroke, id, None);
                }
            }
            ShapeKind::Highlight => {
                if let Some(r) = sk_rect {
                    let mut paint = paint.clone();
                    paint.blend_mode = BlendMode::Multiply;
                    pixmap.fill_rect(r, &paint, id, None);
                }
            }
            ShapeKind::Blur => filter_region(pixmap, shape, |region| {
                image::imageops::fast_blur(region, BLUR_SIGMA)
            }),
            ShapeKind::Pixelate => filter_region(pixmap, shape, pixelate),
            ShapeKind::Text(text) => self.draw_text(pixmap, shape, text),
        }
    }

    fn layout(&mut self, text: &str, font_size: f32) -> Buffer {
        let mut buffer = Buffer::new(
            &mut self.fonts,
            Metrics::new(font_size, (font_size * 1.25).ceil()),
        );
        buffer.set_size(None, None);
        buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.fonts, false);
        buffer
    }

    fn draw_text(&mut self, pixmap: &mut Pixmap, shape: &Shape, text: &str) {
        let mut buffer = self.layout(text, shape.font_size);
        let c = shape.color;
        let base = cosmic_text::Color::rgba(c.r, c.g, c.b, c.a);
        let origin = shape.start;
        buffer.draw(
            &mut self.fonts,
            &mut self.glyphs,
            base,
            |x, y, w, h, color| {
                if color.a() == 0 {
                    return;
                }
                let rect = tiny_skia::Rect::from_xywh(
                    origin.x + x as f32,
                    origin.y + y as f32,
                    w as f32,
                    h as f32,
                );
                if let Some(rect) = rect {
                    let color = Color {
                        r: color.r(),
                        g: color.g(),
                        b: color.b(),
                        a: color.a(),
                    };
                    pixmap.fill_rect(rect, &paint(color), Transform::identity(), None);
                }
            },
        );
    }
}

fn paint(c: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(c.r, c.g, c.b, c.a);
    paint.anti_alias = true;
    paint
}

fn polyline(points: &[Point]) -> Option<tiny_skia::Path> {
    let (first, rest) = points.split_first()?;
    let mut pb = PathBuilder::new();
    pb.move_to(first.x, first.y);
    for p in rest {
        pb.line_to(p.x, p.y);
    }
    pb.finish()
}

fn draw_arrow(pixmap: &mut Pixmap, shape: &Shape, paint: &Paint, stroke: &Stroke) {
    let (a, b) = (shape.start, shape.end);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 {
        return;
    }
    let (ux, uy) = (dx / len, dy / len);
    let head = arrow_head_size(shape.width).min(len);
    // Stop the shaft inside the head so its round cap doesn't poke out.
    let shaft_end = Point::new(b.x - ux * head * 0.8, b.y - uy * head * 0.8);
    if let Some(path) = polyline(&[a, shaft_end]) {
        pixmap.stroke_path(&path, paint, stroke, Transform::identity(), None);
    }
    let back = Point::new(b.x - ux * head, b.y - uy * head);
    let (px, py) = (-uy * head * 0.55, ux * head * 0.55);
    let mut pb = PathBuilder::new();
    pb.move_to(b.x, b.y);
    pb.line_to(back.x + px, back.y + py);
    pb.line_to(back.x - px, back.y - py);
    pb.close();
    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Runs `filter` over the pixels under the shape's rectangle, in place.
/// It sees everything drawn so far, so it hides earlier annotations too.
fn filter_region(pixmap: &mut Pixmap, shape: &Shape, filter: impl Fn(&RgbaImage) -> RgbaImage) {
    let r = shape.rect();
    let (pw, ph) = (pixmap.width(), pixmap.height());
    let x0 = (r.x.floor().max(0.0) as u32).min(pw);
    let y0 = (r.y.floor().max(0.0) as u32).min(ph);
    let x1 = ((r.x + r.width).ceil().max(0.0) as u32).min(pw);
    let y1 = ((r.y + r.height).ceil().max(0.0) as u32).min(ph);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let data = pixmap.data_mut();
    let stride = pw as usize * 4;
    let mut region = RgbaImage::new(w, h);
    for row in 0..h {
        let start = (y0 + row) as usize * stride + x0 as usize * 4;
        let src = &data[start..start + w as usize * 4];
        region.as_mut()[row as usize * w as usize * 4..][..w as usize * 4].copy_from_slice(src);
    }
    let filtered = filter(&region);
    for row in 0..h {
        let start = (y0 + row) as usize * stride + x0 as usize * 4;
        let dst = &mut data[start..start + w as usize * 4];
        dst.copy_from_slice(&filtered.as_raw()[row as usize * w as usize * 4..][..w as usize * 4]);
    }
}

fn pixelate(region: &RgbaImage) -> RgbaImage {
    let (w, h) = region.dimensions();
    let mut out = region.clone();
    for by in (0..h).step_by(PIXEL_BLOCK as usize) {
        for bx in (0..w).step_by(PIXEL_BLOCK as usize) {
            let (bw, bh) = (PIXEL_BLOCK.min(w - bx), PIXEL_BLOCK.min(h - by));
            let mut sum = [0u32; 4];
            for y in by..by + bh {
                for x in bx..bx + bw {
                    for (s, v) in sum.iter_mut().zip(region.get_pixel(x, y).0) {
                        *s += u32::from(v);
                    }
                }
            }
            let n = bw * bh;
            let avg = image::Rgba(sum.map(|s| (s / n) as u8));
            for y in by..by + bh {
                for x in bx..bx + bw {
                    out.put_pixel(x, y, avg);
                }
            }
        }
    }
    out
}

fn to_pixmap(image: &RgbaImage) -> Pixmap {
    let mut pixmap = Pixmap::new(image.width().max(1), image.height().max(1)).expect("non-empty");
    for (dst, src) in pixmap.pixels_mut().iter_mut().zip(image.pixels()) {
        let [r, g, b, a] = src.0;
        *dst = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
    }
    pixmap
}

fn to_image(pixmap: &Pixmap) -> RgbaImage {
    let mut image = RgbaImage::new(pixmap.width(), pixmap.height());
    for (dst, src) in image.pixels_mut().zip(pixmap.pixels()) {
        let c = PremultipliedColorU8::demultiply(src);
        *dst = image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::Rect;

    fn checker(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            if (x + y) % 2 == 0 {
                image::Rgba([255, 255, 255, 255])
            } else {
                image::Rgba([0, 0, 0, 255])
            }
        })
    }

    fn white(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]))
    }

    #[test]
    fn untouched_document_exports_unchanged() {
        let base = checker(40, 30);
        let doc = Document::new(base.clone());
        assert_eq!(Renderer::new().export(&doc), base);
    }

    #[test]
    fn rect_is_drawn_on_its_border() {
        let mut doc = Document::new(white(60, 60));
        let mut s = Shape::new(
            ShapeKind::Rect,
            Point::new(10.0, 10.0),
            Point::new(50.0, 50.0),
        );
        s.color = Color::rgb(255, 0, 0);
        doc.add(s);
        let out = Renderer::new().export(&doc);
        assert_eq!(out.get_pixel(10, 30).0, [255, 0, 0, 255]);
        assert_eq!(out.get_pixel(30, 30).0, [255, 255, 255, 255]);
    }

    #[test]
    fn pixelate_flattens_blocks() {
        let mut doc = Document::new(checker(48, 48));
        doc.add(Shape::new(
            ShapeKind::Pixelate,
            Point::new(0.0, 0.0),
            Point::new(24.0, 24.0),
        ));
        let out = Renderer::new().export(&doc);
        let first = out.get_pixel(0, 0);
        for y in 0..12 {
            for x in 0..12 {
                assert_eq!(out.get_pixel(x, y), first);
            }
        }
        // Outside the shape the checkerboard is untouched.
        assert_ne!(out.get_pixel(30, 30), out.get_pixel(31, 30));
    }

    #[test]
    fn blur_hides_earlier_annotations() {
        let mut doc = Document::new(white(80, 80));
        let mut line = Shape::new(
            ShapeKind::Line,
            Point::new(10.0, 40.0),
            Point::new(70.0, 40.0),
        );
        line.color = Color::rgb(0, 0, 0);
        line.width = 2.0;
        doc.add(line);
        doc.add(Shape::new(
            ShapeKind::Blur,
            Point::new(0.0, 0.0),
            Point::new(80.0, 80.0),
        ));
        let out = Renderer::new().export(&doc);
        let darkest = out.pixels().map(|p| p.0[0]).min().unwrap();
        assert!(
            darkest > 100,
            "the line should be smeared out, got {darkest}"
        );
    }

    #[test]
    fn highlight_multiplies() {
        let mut doc = Document::new(white(20, 20));
        let mut s = Shape::new(
            ShapeKind::Highlight,
            Point::new(0.0, 0.0),
            Point::new(20.0, 20.0),
        );
        s.color = Color::rgb(255, 230, 0);
        doc.add(s);
        let out = Renderer::new().export(&doc);
        assert_eq!(out.get_pixel(5, 5).0, [255, 230, 0, 255]);
    }

    #[test]
    fn crop_applies_on_export() {
        let mut doc = Document::new(checker(40, 30));
        doc.set_crop(Some(Rect {
            x: 5.0,
            y: 5.0,
            width: 10.0,
            height: 8.0,
        }));
        let out = Renderer::new().export(&doc);
        assert_eq!(out.dimensions(), (10, 8));
    }

    #[test]
    fn preview_does_not_touch_the_document() {
        let mut doc = Document::new(white(30, 30));
        let mut r = Renderer::new();
        let s = Shape::new(
            ShapeKind::Line,
            Point::new(0.0, 15.0),
            Point::new(30.0, 15.0),
        );
        let with = r.render(&doc, Some(&s));
        let without = r.render(&doc, None);
        assert_ne!(with.data(), without.data());
        doc.add(s);
        assert_eq!(r.render(&doc, None).data(), with.data());
    }

    #[test]
    fn text_is_measured_and_drawn() {
        let mut r = Renderer::new();
        let (w, h) = r.measure_text("Hello", 24.0);
        if w == 0.0 {
            // No fonts on this machine (minimal CI image): nothing to check.
            return;
        }
        assert!(h >= 24.0);
        let mut doc = Document::new(white(200, 60));
        let mut s = Shape::new(
            ShapeKind::Text("Hello".into()),
            Point::new(5.0, 5.0),
            Point::new(5.0 + w, 5.0 + h),
        );
        s.color = Color::rgb(0, 0, 0);
        doc.add(s);
        let out = r.export(&doc);
        assert!(out.pixels().any(|p| p.0[0] < 128));
    }
}
