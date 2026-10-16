//! The editor's document: annotations drawn over a capture, with undo.
//!
//! Coordinates are in image pixels. Rendering lives in [`render`] and is the
//! same for the live view and the exported image.

pub mod render;

use image::RgbaImage;

use crate::output::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const RED: Color = Color::rgb(0xe5, 0x39, 0x35);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Parses `#rrggbb` or `#rrggbbaa`.
    pub fn parse(s: &str) -> Option<Self> {
        let hex = s.strip_prefix('#')?;
        let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        match hex.len() {
            6 => Some(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Self {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: byte(6)?,
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Line,
    Arrow,
    /// A freehand stroke through these points.
    Freehand(Vec<Point>),
    /// Highlighter: multiplies the color over the area.
    Highlight,
    Blur,
    Pixelate,
    /// Text whose top-left corner is `start`.
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub kind: ShapeKind,
    /// First corner, or line start.
    pub start: Point,
    /// Opposite corner, or line end.
    pub end: Point,
    pub color: Color,
    /// Stroke width in pixels.
    pub width: f32,
    /// Text size in pixels.
    pub font_size: f32,
}

impl Shape {
    pub fn new(kind: ShapeKind, start: Point, end: Point) -> Self {
        Self {
            kind,
            start,
            end,
            color: Color::RED,
            width: 4.0,
            font_size: 28.0,
        }
    }

    /// The rectangle spanned by `start` and `end`, normalised.
    pub fn rect(&self) -> Rect {
        rect_between(self.start, self.end)
    }

    /// Rough bounds, used for hit testing and the selection outline. Text
    /// bounds come from the renderer, which knows the font metrics.
    pub fn bounds(&self) -> Rect {
        let pad = f64::from(self.width) / 2.0;
        let r = match &self.kind {
            ShapeKind::Freehand(points) => {
                let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for p in points {
                    x0 = x0.min(p.x);
                    y0 = y0.min(p.y);
                    x1 = x1.max(p.x);
                    y1 = y1.max(p.y);
                }
                rect_between(Point::new(x0, y0), Point::new(x1, y1))
            }
            ShapeKind::Arrow => {
                let r = self.rect();
                let head = f64::from(arrow_head_size(self.width));
                return inflate(r, head);
            }
            _ => self.rect(),
        };
        inflate(r, pad)
    }

    /// Whether `p` is on (or, for filled kinds, inside) the shape.
    pub fn hit(&self, p: Point, tolerance: f32) -> bool {
        let near = tolerance + self.width / 2.0;
        match &self.kind {
            ShapeKind::Line | ShapeKind::Arrow => {
                distance_to_segment(p, self.start, self.end) <= near
            }
            ShapeKind::Freehand(points) => points
                .windows(2)
                .any(|w| distance_to_segment(p, w[0], w[1]) <= near),
            ShapeKind::Rect | ShapeKind::Ellipse => {
                let r = self.rect();
                let inside = contains(inflate(r, f64::from(near)), p);
                let deep_inside = contains(inflate(r, -f64::from(near)), p);
                inside && !deep_inside
            }
            ShapeKind::Highlight | ShapeKind::Blur | ShapeKind::Pixelate | ShapeKind::Text(_) => {
                contains(inflate(self.bounds(), f64::from(tolerance)), p)
            }
        }
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.start.x += dx;
        self.start.y += dy;
        self.end.x += dx;
        self.end.y += dy;
        if let ShapeKind::Freehand(points) = &mut self.kind {
            for p in points {
                p.x += dx;
                p.y += dy;
            }
        }
    }
}

pub(crate) fn arrow_head_size(width: f32) -> f32 {
    (width * 4.0).max(14.0)
}

fn rect_between(a: Point, b: Point) -> Rect {
    Rect {
        x: f64::from(a.x.min(b.x)),
        y: f64::from(a.y.min(b.y)),
        width: f64::from((a.x - b.x).abs()),
        height: f64::from((a.y - b.y).abs()),
    }
}

fn inflate(r: Rect, by: f64) -> Rect {
    Rect {
        x: r.x - by,
        y: r.y - by,
        width: (r.width + 2.0 * by).max(0.0),
        height: (r.height + 2.0 * by).max(0.0),
    }
}

fn contains(r: Rect, p: Point) -> bool {
    let (x, y) = (f64::from(p.x), f64::from(p.y));
    x >= r.x && y >= r.y && x <= r.x + r.width && y <= r.y + r.height
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    let (cx, cy) = (a.x + t * dx, a.y + t * dy);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

#[derive(Debug, Clone, PartialEq, Default)]
struct State {
    shapes: Vec<Shape>,
    crop: Option<Rect>,
}

/// A capture plus its annotations.
pub struct Document {
    base: RgbaImage,
    state: State,
    undo: Vec<State>,
    redo: Vec<State>,
    /// Bumped on every change, so renderers know when to redraw.
    revision: u64,
}

impl Document {
    pub fn new(base: RgbaImage) -> Self {
        Self {
            base,
            state: State::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
        }
    }

    pub fn base(&self) -> &RgbaImage {
        &self.base
    }

    pub fn shapes(&self) -> &[Shape] {
        &self.state.shapes
    }

    pub fn crop(&self) -> Option<Rect> {
        self.state.crop
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Saves the current state for undo. Call once before a group of edits,
    /// for example at the start of a drag.
    pub fn checkpoint(&mut self) {
        self.undo.push(self.state.clone());
        self.redo.clear();
    }

    pub fn add(&mut self, shape: Shape) {
        self.checkpoint();
        self.state.shapes.push(shape);
        self.revision += 1;
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.state.shapes.len() {
            self.checkpoint();
            self.state.shapes.remove(index);
            self.revision += 1;
        }
    }

    /// Moves a shape without recording undo; pair with [`Self::checkpoint`].
    pub fn translate(&mut self, index: usize, dx: f32, dy: f32) {
        if let Some(shape) = self.state.shapes.get_mut(index) {
            shape.translate(dx, dy);
            self.revision += 1;
        }
    }

    pub fn set_crop(&mut self, crop: Option<Rect>) {
        self.checkpoint();
        self.state.crop = crop.map(|c| self.clamp(c));
        self.revision += 1;
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.state, prev));
            self.revision += 1;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.state, next));
            self.revision += 1;
        }
    }

    /// The topmost shape under `p`.
    pub fn hit_test(&self, p: Point, tolerance: f32) -> Option<usize> {
        self.state.shapes.iter().rposition(|s| s.hit(p, tolerance))
    }

    fn clamp(&self, r: Rect) -> Rect {
        let (w, h) = (f64::from(self.base.width()), f64::from(self.base.height()));
        let x = r.x.clamp(0.0, w);
        let y = r.y.clamp(0.0, h);
        Rect {
            x,
            y,
            width: r.width.min(w - x).max(0.0),
            height: r.height.min(h - y).max(0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Document {
        Document::new(RgbaImage::new(200, 100))
    }

    fn rect_shape() -> Shape {
        Shape::new(
            ShapeKind::Rect,
            Point::new(10.0, 10.0),
            Point::new(50.0, 40.0),
        )
    }

    #[test]
    fn undo_and_redo() {
        let mut d = doc();
        d.add(rect_shape());
        d.add(Shape::new(
            ShapeKind::Line,
            Point::new(0.0, 0.0),
            Point::new(5.0, 5.0),
        ));
        assert_eq!(d.shapes().len(), 2);
        d.undo();
        assert_eq!(d.shapes().len(), 1);
        d.undo();
        assert!(d.shapes().is_empty());
        assert!(!d.can_undo());
        d.redo();
        assert_eq!(d.shapes().len(), 1);
        d.add(rect_shape());
        assert!(!d.can_redo(), "a new edit clears redo");
    }

    #[test]
    fn a_drag_is_one_undo_step() {
        let mut d = doc();
        d.add(rect_shape());
        d.checkpoint();
        for _ in 0..10 {
            d.translate(0, 1.0, 0.0);
        }
        assert_eq!(d.shapes()[0].start.x, 20.0);
        d.undo();
        assert_eq!(d.shapes()[0].start.x, 10.0);
    }

    #[test]
    fn rect_hits_on_its_border_only() {
        let s = rect_shape();
        assert!(s.hit(Point::new(10.0, 25.0), 2.0));
        assert!(!s.hit(Point::new(30.0, 25.0), 2.0));
        assert!(!s.hit(Point::new(100.0, 25.0), 2.0));
    }

    #[test]
    fn hit_test_prefers_topmost() {
        let mut d = doc();
        d.add(Shape::new(
            ShapeKind::Blur,
            Point::new(0.0, 0.0),
            Point::new(100.0, 100.0),
        ));
        d.add(Shape::new(
            ShapeKind::Highlight,
            Point::new(20.0, 20.0),
            Point::new(60.0, 60.0),
        ));
        assert_eq!(d.hit_test(Point::new(30.0, 30.0), 2.0), Some(1));
        assert_eq!(d.hit_test(Point::new(80.0, 80.0), 2.0), Some(0));
        assert_eq!(d.hit_test(Point::new(150.0, 80.0), 2.0), None);
    }

    #[test]
    fn lines_hit_near_the_segment() {
        let s = Shape::new(
            ShapeKind::Arrow,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
        );
        assert!(s.hit(Point::new(50.0, 3.0), 2.0));
        assert!(!s.hit(Point::new(50.0, 20.0), 2.0));
    }

    #[test]
    fn crop_is_clamped_to_the_image() {
        let mut d = doc();
        d.set_crop(Some(Rect {
            x: 150.0,
            y: -10.0,
            width: 100.0,
            height: 50.0,
        }));
        assert_eq!(
            d.crop(),
            Some(Rect {
                x: 150.0,
                y: 0.0,
                width: 50.0,
                height: 50.0
            })
        );
        d.undo();
        assert_eq!(d.crop(), None);
    }

    #[test]
    fn parses_colors() {
        assert_eq!(Color::parse("#ff8000"), Some(Color::rgb(255, 128, 0)));
        assert_eq!(Color::parse("#ff800080").map(|c| c.a), Some(128));
        assert_eq!(Color::parse("ff8000"), None);
        assert_eq!(Color::parse("#ff80"), None);
    }
}
