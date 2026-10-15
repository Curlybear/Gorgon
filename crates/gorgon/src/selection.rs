//! Dims the frozen screen outside the selection, drawn from Rust with QPainter.
//! The editor will render annotations the same way.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
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
        #[qproperty(QRectF, selection)]
        type SelectionPainter = super::SelectionPainterRust;

        #[qinvokable]
        #[cxx_override]
        unsafe fn paint(self: Pin<&mut Self>, painter: *mut QPainter);

        #[inherit]
        fn size(self: &Self) -> QSizeF;

        #[inherit]
        fn update(self: Pin<&mut Self>);
    }

    impl cxx_qt::Initialize for SelectionPainter {}
}

use std::pin::Pin;

use cxx_qt_lib::{QColor, QRectF};

#[derive(Default)]
pub struct SelectionPainterRust {
    selection: QRectF,
}

impl qobject::SelectionPainter {
    /// # Safety
    ///
    /// Qt passes a valid painter for the duration of the call.
    pub unsafe fn paint(self: Pin<&mut Self>, painter: *mut qobject::QPainter) {
        let Some(painter) = painter.as_mut() else {
            return;
        };
        let mut p = Pin::new_unchecked(painter);
        let size = self.size();
        let (w, h) = (size.width(), size.height());
        let sel = self.selection();
        let (sx, sy, sw, sh) = (sel.x(), sel.y(), sel.width(), sel.height());
        let dim = QColor::from_rgba_f(0.0, 0.0, 0.0, 0.45);

        if sw <= 0.0 || sh <= 0.0 {
            p.as_mut().fill_rect(&QRectF::new(0.0, 0.0, w, h), &dim);
            return;
        }
        // Four bands around the selection, so the selection itself stays undimmed.
        p.as_mut().fill_rect(&QRectF::new(0.0, 0.0, w, sy), &dim);
        p.as_mut()
            .fill_rect(&QRectF::new(0.0, sy + sh, w, h - sy - sh), &dim);
        p.as_mut().fill_rect(&QRectF::new(0.0, sy, sx, sh), &dim);
        p.as_mut()
            .fill_rect(&QRectF::new(sx + sw, sy, w - sx - sw, sh), &dim);

        let border = QColor::from_rgb(64, 160, 255);
        p.as_mut()
            .fill_rect(&QRectF::new(sx - 1.0, sy - 1.0, sw + 2.0, 1.0), &border);
        p.as_mut()
            .fill_rect(&QRectF::new(sx - 1.0, sy + sh, sw + 2.0, 1.0), &border);
        p.as_mut()
            .fill_rect(&QRectF::new(sx - 1.0, sy, 1.0, sh), &border);
        p.as_mut()
            .fill_rect(&QRectF::new(sx + sw, sy, 1.0, sh), &border);
    }
}

impl cxx_qt::Initialize for qobject::SelectionPainter {
    fn initialize(self: Pin<&mut Self>) {
        self.on_selection_changed(|item| item.update()).release();
    }
}
