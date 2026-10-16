//! Rough render timings at a laptop screen size: `cargo run --release --example render_bench`.

use std::time::Instant;

use gorgon_core::annotate::{render::Renderer, Document, Point, Shape, ShapeKind};
use image::RgbaImage;

fn main() {
    let base = RgbaImage::from_fn(2256, 1504, |x, y| {
        image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
    });
    let mut doc = Document::new(base);
    let start = Instant::now();
    let mut r = Renderer::new();
    println!("font load: {:?}", start.elapsed());
    for (kind, a, b) in [
        (ShapeKind::Rect, (100.0, 100.0), (600.0, 400.0)),
        (ShapeKind::Arrow, (700.0, 700.0), (1200.0, 500.0)),
        (ShapeKind::Blur, (200.0, 800.0), (900.0, 1200.0)),
        (ShapeKind::Pixelate, (1300.0, 800.0), (2000.0, 1200.0)),
        (
            ShapeKind::Text("Hello from Gorgon".into()),
            (100.0, 50.0),
            (500.0, 90.0),
        ),
    ] {
        doc.add(Shape::new(kind, Point::new(a.0, a.1), Point::new(b.0, b.1)));
    }
    let t = Instant::now();
    r.render(&doc, None);
    println!("full render: {:?}", t.elapsed());
    let preview = Shape::new(
        ShapeKind::Rect,
        Point::new(10.0, 10.0),
        Point::new(300.0, 300.0),
    );
    let t = Instant::now();
    for _ in 0..10 {
        r.render(&doc, Some(&preview));
    }
    println!("cached render + preview: {:?} each", t.elapsed() / 10);
    let t = Instant::now();
    r.export(&doc);
    println!("export: {:?}", t.elapsed());
}
