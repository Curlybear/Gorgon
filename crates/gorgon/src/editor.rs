//! The editor canvas: shows a capture with its annotations and turns mouse
//! input into shapes. Drawing itself is done by gorgon-core's renderer.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qrectf.h");
        type QRectF = cxx_qt_lib::QRectF;
        include!("cxx-qt-lib/qsizef.h");
        type QSizeF = cxx_qt_lib::QSizeF;
        include!("cxx-qt-lib/qpainter.h");
        type QPainter = cxx_qt_lib::QPainter;
    }

    unsafe extern "C++" {
        include!(<QtQuick/QQuickPaintedItem>);
        type QQuickPaintedItem;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QQuickPaintedItem]
        #[qproperty(QString, tool)]
        #[qproperty(QString, color)]
        #[qproperty(f64, stroke_width, cxx_name = "strokeWidth")]
        #[qproperty(f64, font_size, cxx_name = "fontSize")]
        #[qproperty(f64, pixel_ratio, cxx_name = "pixelRatio")]
        #[qproperty(bool, can_undo, cxx_name = "canUndo")]
        #[qproperty(bool, can_redo, cxx_name = "canRedo")]
        #[qproperty(bool, has_selection, cxx_name = "hasSelection")]
        #[qproperty(i32, image_width, cxx_name = "imageWidth")]
        #[qproperty(i32, image_height, cxx_name = "imageHeight")]
        type EditorCanvas = super::EditorCanvasRust;

        /// The text tool was clicked here (item coordinates); QML shows an input.
        #[qsignal]
        #[cxx_name = "textRequested"]
        fn text_requested(self: Pin<&mut EditorCanvas>, x: f64, y: f64);

        /// Takes the next capture waiting for an editor. False if there is none.
        #[qinvokable]
        fn load(self: Pin<&mut EditorCanvas>) -> bool;

        #[qinvokable]
        fn press(self: Pin<&mut EditorCanvas>, x: f64, y: f64);

        #[qinvokable]
        #[cxx_name = "moveTo"]
        fn move_to(self: Pin<&mut EditorCanvas>, x: f64, y: f64);

        #[qinvokable]
        fn release(self: Pin<&mut EditorCanvas>, x: f64, y: f64);

        #[qinvokable]
        #[cxx_name = "addText"]
        fn add_text(self: Pin<&mut EditorCanvas>, x: f64, y: f64, text: &QString);

        #[qinvokable]
        fn undo(self: Pin<&mut EditorCanvas>);

        #[qinvokable]
        fn redo(self: Pin<&mut EditorCanvas>);

        #[qinvokable]
        #[cxx_name = "deleteSelected"]
        fn delete_selected(self: Pin<&mut EditorCanvas>);

        /// Flattens the annotations, copies and saves the result.
        #[qinvokable]
        fn finish(self: Pin<&mut EditorCanvas>);

        #[qinvokable]
        #[cxx_override]
        unsafe fn paint(self: Pin<&mut EditorCanvas>, painter: *mut QPainter);

        #[inherit]
        fn size(self: &EditorCanvas) -> QSizeF;

        #[inherit]
        fn update(self: Pin<&mut EditorCanvas>);
    }

    impl cxx_qt::Initialize for EditorCanvas {}
}

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QColor, QImage, QImageFormat, QPainterRenderHint, QRect, QRectF, QString};
use gorgon_core::{
    annotate::{render::Renderer, Color, Document, Point, Shape, ShapeKind},
    config::Config,
    output::Rect,
};

/// A capture being edited.
struct Session {
    doc: Document,
    renderer: Renderer,
    config: Config,
}

enum Drag {
    None,
    /// Drawing `preview`.
    Draw,
    /// Moving the selected shape; the last pointer position in image pixels.
    Move(Point),
    /// Dragging out a crop from this corner.
    Crop(Point),
}

pub struct EditorCanvasRust {
    tool: QString,
    color: QString,
    stroke_width: f64,
    font_size: f64,
    /// Device pixels per logical pixel, so captures show 1:1 at most.
    pixel_ratio: f64,
    can_undo: bool,
    can_redo: bool,
    has_selection: bool,
    image_width: i32,
    image_height: i32,
    session: Option<Session>,
    drag: Drag,
    preview: Option<Shape>,
    crop_preview: Option<Rect>,
    selected: Option<usize>,
}

impl Default for EditorCanvasRust {
    fn default() -> Self {
        Self {
            tool: QString::from("rect"),
            color: QString::from("#e53935"),
            stroke_width: 4.0,
            font_size: 28.0,
            pixel_ratio: 1.0,
            can_undo: false,
            can_redo: false,
            has_selection: false,
            image_width: 0,
            image_height: 0,
            session: None,
            drag: Drag::None,
            preview: None,
            crop_preview: None,
            selected: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::EditorCanvas {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut()
            .on_pixel_ratio_changed(|canvas| canvas.update())
            .release();
        self.on_tool_changed(|mut canvas| {
            // Switching tools drops the selection.
            canvas.as_mut().rust_mut().selected = None;
            canvas.as_mut().set_has_selection(false);
            canvas.update();
        })
        .release();
    }
}

/// Image-space placement of the capture inside the item: fit to the item,
/// never shown larger than one image pixel per logical pixel.
#[derive(Clone, Copy)]
struct View {
    scale: f64,
    x: f64,
    y: f64,
}

impl View {
    fn to_image(self, x: f64, y: f64) -> Point {
        Point::new(
            ((x - self.x) / self.scale) as f32,
            ((y - self.y) / self.scale) as f32,
        )
    }

    fn to_item(self, r: Rect) -> QRectF {
        QRectF::new(
            self.x + r.x * self.scale,
            self.y + r.y * self.scale,
            r.width * self.scale,
            r.height * self.scale,
        )
    }
}

fn tool_kind(tool: &str) -> Option<ShapeKind> {
    Some(match tool {
        "rect" => ShapeKind::Rect,
        "ellipse" => ShapeKind::Ellipse,
        "line" => ShapeKind::Line,
        "arrow" => ShapeKind::Arrow,
        "pen" => ShapeKind::Freehand(Vec::new()),
        "highlight" => ShapeKind::Highlight,
        "blur" => ShapeKind::Blur,
        "pixelate" => ShapeKind::Pixelate,
        _ => return None,
    })
}

fn normalized(a: Point, b: Point) -> Rect {
    Rect {
        x: f64::from(a.x.min(b.x)),
        y: f64::from(a.y.min(b.y)),
        width: f64::from((a.x - b.x).abs()),
        height: f64::from((a.y - b.y).abs()),
    }
}

impl qobject::EditorCanvas {
    fn view(&self) -> Option<View> {
        let (w, h) = (f64::from(self.image_width), f64::from(self.image_height));
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let size = self.size();
        // Fill the item; only small captures in a big window stop growing,
        // at one image pixel per logical pixel.
        let scale = (size.width() / w).min(size.height() / h).min(1.0);
        Some(View {
            scale,
            x: ((size.width() - w * scale) / 2.0).round(),
            y: ((size.height() - h * scale) / 2.0).round(),
        })
    }

    fn new_shape(&self, kind: ShapeKind, at: Point) -> Shape {
        let mut shape = Shape::new(kind, at, at);
        shape.color = Color::parse(&self.color.to_string()).unwrap_or(Color::RED);
        if matches!(shape.kind, ShapeKind::Highlight) && self.color.to_string() == "#e53935" {
            // Red highlighter is rarely wanted; default to yellow.
            shape.color = Color::rgb(0xff, 0xe0, 0x30);
        }
        shape.width = *self.stroke_width() as f32;
        shape.font_size = *self.font_size() as f32;
        if let ShapeKind::Freehand(points) = &mut shape.kind {
            points.push(at);
        }
        shape
    }

    /// Keeps the QML-facing flags in sync after any change.
    fn changed(mut self: Pin<&mut Self>) {
        let (can_undo, can_redo) = self
            .session
            .as_ref()
            .map_or((false, false), |s| (s.doc.can_undo(), s.doc.can_redo()));
        let has_selection = self.selected.is_some();
        self.as_mut().set_can_undo(can_undo);
        self.as_mut().set_can_redo(can_redo);
        self.as_mut().set_has_selection(has_selection);
        self.update();
    }

    pub fn load(mut self: Pin<&mut Self>) -> bool {
        let Some((image, config)) = crate::app::take_pending_edit() else {
            return false;
        };
        let (w, h) = (image.width() as i32, image.height() as i32);
        self.as_mut().rust_mut().session = Some(Session {
            doc: Document::new(image),
            renderer: Renderer::new(),
            config,
        });
        self.as_mut().set_image_width(w);
        self.as_mut().set_image_height(h);
        self.changed();
        true
    }

    pub fn press(mut self: Pin<&mut Self>, x: f64, y: f64) {
        let Some(view) = self.view() else { return };
        let p = view.to_image(x, y);
        let tool = self.tool().to_string();
        match tool.as_str() {
            "select" => {
                let tolerance = (6.0 / view.scale) as f32;
                let mut rust = self.as_mut().rust_mut();
                let hit = rust
                    .session
                    .as_ref()
                    .and_then(|s| s.doc.hit_test(p, tolerance));
                rust.selected = hit;
                if hit.is_some() {
                    if let Some(s) = rust.session.as_mut() {
                        s.doc.checkpoint();
                    }
                    rust.drag = Drag::Move(p);
                }
            }
            "crop" => self.as_mut().rust_mut().drag = Drag::Crop(p),
            "text" => {
                self.as_mut().text_requested(x, y);
                return;
            }
            other => {
                if let Some(kind) = tool_kind(other) {
                    let shape = self.new_shape(kind, p);
                    let mut rust = self.as_mut().rust_mut();
                    rust.preview = Some(shape);
                    rust.drag = Drag::Draw;
                }
            }
        }
        self.changed();
    }

    pub fn move_to(mut self: Pin<&mut Self>, x: f64, y: f64) {
        let Some(view) = self.view() else { return };
        let p = view.to_image(x, y);
        let mut rust = self.as_mut().rust_mut();
        match rust.drag {
            Drag::None => return,
            Drag::Draw => {
                if let Some(shape) = rust.preview.as_mut() {
                    match &mut shape.kind {
                        ShapeKind::Freehand(points) => {
                            let last = points.last().copied().unwrap_or(p);
                            if (last.x - p.x).abs() + (last.y - p.y).abs() >= 1.0 {
                                points.push(p);
                            }
                        }
                        _ => shape.end = p,
                    }
                }
            }
            Drag::Move(last) => {
                if let (Some(i), Some(s)) = (rust.selected, rust.session.as_mut()) {
                    s.doc.translate(i, p.x - last.x, p.y - last.y);
                }
                rust.drag = Drag::Move(p);
            }
            Drag::Crop(start) => rust.crop_preview = Some(normalized(start, p)),
        }
        self.update();
    }

    pub fn release(mut self: Pin<&mut Self>, x: f64, y: f64) {
        self.as_mut().move_to(x, y);
        let mut rust = self.as_mut().rust_mut();
        let drag = std::mem::replace(&mut rust.drag, Drag::None);
        match drag {
            Drag::Draw => {
                if let (Some(shape), Some(s)) = (rust.preview.take(), rust.session.as_mut()) {
                    let r = shape.rect();
                    let big_enough = matches!(shape.kind, ShapeKind::Freehand(_))
                        || r.width >= 3.0
                        || r.height >= 3.0;
                    if big_enough {
                        s.doc.add(shape);
                    }
                }
            }
            Drag::Crop(_) => {
                if let (Some(crop), Some(s)) = (rust.crop_preview.take(), rust.session.as_mut()) {
                    if crop.width >= 4.0 && crop.height >= 4.0 {
                        s.doc.set_crop(Some(crop));
                    }
                }
            }
            Drag::Move(_) | Drag::None => {}
        }
        self.changed();
    }

    pub fn add_text(mut self: Pin<&mut Self>, x: f64, y: f64, text: &QString) {
        let text = text.to_string();
        if text.trim().is_empty() {
            return;
        }
        let Some(view) = self.view() else { return };
        let p = view.to_image(x, y);
        let mut shape = self.new_shape(ShapeKind::Text(text.clone()), p);
        if let Some(s) = self.as_mut().rust_mut().session.as_mut() {
            let (w, h) = s.renderer.measure_text(&text, shape.font_size);
            shape.end = Point::new(p.x + w, p.y + h);
            s.doc.add(shape);
        }
        self.changed();
    }

    pub fn undo(mut self: Pin<&mut Self>) {
        let mut rust = self.as_mut().rust_mut();
        rust.selected = None;
        if let Some(s) = rust.session.as_mut() {
            s.doc.undo();
        }
        self.changed();
    }

    pub fn redo(mut self: Pin<&mut Self>) {
        let mut rust = self.as_mut().rust_mut();
        rust.selected = None;
        if let Some(s) = rust.session.as_mut() {
            s.doc.redo();
        }
        self.changed();
    }

    pub fn delete_selected(mut self: Pin<&mut Self>) {
        let mut rust = self.as_mut().rust_mut();
        if let (Some(i), Some(s)) = (rust.selected.take(), rust.session.as_mut()) {
            s.doc.remove(i);
        }
        self.changed();
    }

    pub fn finish(mut self: Pin<&mut Self>) {
        let Some(mut session) = self.as_mut().rust_mut().session.take() else {
            return;
        };
        let image = session.renderer.export(&session.doc);
        // The editor window has focus right now, so the clipboard works everywhere.
        let copied = session.config.copy_image && crate::app::copy_to_clipboard(&image);
        crate::app::deliver(image, session.config, copied);
    }

    /// # Safety
    ///
    /// Qt passes a valid painter for the duration of the call.
    pub unsafe fn paint(mut self: Pin<&mut Self>, painter: *mut qobject::QPainter) {
        let Some(painter) = painter.as_mut() else {
            return;
        };
        let mut p = Pin::new_unchecked(painter);
        let Some(view) = self.view() else { return };
        let (w, h) = (self.image_width, self.image_height);

        let mut rust = self.as_mut().rust_mut();
        let rust = &mut *rust;
        let Some(session) = rust.session.as_mut() else {
            return;
        };
        let pixmap = session.renderer.render(&session.doc, rust.preview.as_ref());
        let image = QImage::from_raw_bytes(
            pixmap.take(),
            w,
            h,
            QImageFormat::Format_RGBA8888_Premultiplied,
        );
        let target = QRect::new(
            view.x as i32,
            view.y as i32,
            (f64::from(w) * view.scale).round() as i32,
            (f64::from(h) * view.scale).round() as i32,
        );
        p.as_mut()
            .set_render_hint(QPainterRenderHint::SmoothPixmapTransform, true);
        p.as_mut().draw_image(&target, &image);

        // Dim what the crop will cut away.
        if let Some(crop) = rust.crop_preview.or(session.doc.crop()) {
            let full = view.to_item(Rect {
                x: 0.0,
                y: 0.0,
                width: f64::from(w),
                height: f64::from(h),
            });
            let c = view.to_item(crop);
            let dim = QColor::from_rgba_f(0.0, 0.0, 0.0, 0.55);
            let (fx, fy, fw, fh) = (full.x(), full.y(), full.width(), full.height());
            let (cx, cy, cw, ch) = (c.x(), c.y(), c.width(), c.height());
            p.as_mut()
                .fill_rect(&QRectF::new(fx, fy, fw, cy - fy), &dim);
            p.as_mut()
                .fill_rect(&QRectF::new(fx, cy + ch, fw, fy + fh - cy - ch), &dim);
            p.as_mut()
                .fill_rect(&QRectF::new(fx, cy, cx - fx, ch), &dim);
            p.as_mut()
                .fill_rect(&QRectF::new(cx + cw, cy, fx + fw - cx - cw, ch), &dim);
            outline(p.as_mut(), &c, &QColor::from_rgb(255, 255, 255));
        }

        if let Some(shape) = rust.selected.and_then(|i| session.doc.shapes().get(i)) {
            outline(
                p.as_mut(),
                &view.to_item(shape.bounds()),
                &QColor::from_rgb(64, 160, 255),
            );
        }
    }
}

/// A 1px rectangle outline, drawn with fills since no pen is needed.
fn outline(mut p: Pin<&mut qobject::QPainter>, r: &QRectF, color: &QColor) {
    let (x, y, w, h) = (r.x(), r.y(), r.width(), r.height());
    p.as_mut()
        .fill_rect(&QRectF::new(x - 1.0, y - 1.0, w + 2.0, 1.0), color);
    p.as_mut()
        .fill_rect(&QRectF::new(x - 1.0, y + h, w + 2.0, 1.0), color);
    p.as_mut()
        .fill_rect(&QRectF::new(x - 1.0, y, 1.0, h), color);
    p.as_mut().fill_rect(&QRectF::new(x + w, y, 1.0, h), color);
}
