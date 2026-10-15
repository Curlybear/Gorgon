use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("dev.gorgon.spike")
            .qml_file("qml/main.qml")
            .qml_file("qml/Overlay.qml")
            .qml_file("qml/Tray.qml"),
    )
    .qt_module("Quick")
    .files(["src/capture.rs", "src/selection.rs"])
    .build();
}
