use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("dev.gorgon")
            .qml_file("qml/main.qml")
            .qml_file("qml/RegionOverlay.qml")
            .qml_file("qml/Flash.qml")
            .qml_file("qml/EditorWindow.qml")
            .qml_file("qml/ToolButton.qml")
            .qml_file("qml/ClipboardGrab.qml"),
    )
    .qt_module("Quick")
    .files(["src/app.rs", "src/editor.rs", "src/selection.rs"])
    .include_dir("cpp")
    .cpp_file("cpp/qt_helpers.cpp")
    .build();
}
