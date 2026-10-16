#include "gorgon/qt_helpers.h"

#include <QClipboard>
#include <QGuiApplication>
#include <QIcon>
#include <QImage>
#include <QString>

namespace gorgon {

bool set_clipboard_image(rust::Slice<const std::uint8_t> rgba, std::int32_t width,
                         std::int32_t height) {
  if (width <= 0 || height <= 0 ||
      rgba.size() != static_cast<std::size_t>(width) * static_cast<std::size_t>(height) * 4) {
    return false;
  }
  QClipboard* clipboard = QGuiApplication::clipboard();
  if (clipboard == nullptr) {
    return false;
  }
  const QImage view(rgba.data(), width, height, width * 4, QImage::Format_RGBA8888);
  // copy(): the clipboard must own its pixels, the Rust buffer goes away.
  clipboard->setImage(view.copy());
  return true;
}

void set_clipboard_text(rust::Str text) {
  if (QClipboard* clipboard = QGuiApplication::clipboard()) {
    clipboard->setText(QString::fromUtf8(text.data(), static_cast<qsizetype>(text.size())));
  }
}

void keep_running_without_windows() {
  QGuiApplication::setQuitOnLastWindowClosed(false);
}

void set_app_identity(rust::Str desktop_file, rust::Str icon_path) {
  const auto name = QString::fromUtf8(desktop_file.data(), desktop_file.size());
  QGuiApplication::setDesktopFileName(name);
  QIcon icon = QIcon::fromTheme(name);
  if (icon.isNull())
    icon = QIcon(QString::fromUtf8(icon_path.data(), icon_path.size()));
  QGuiApplication::setWindowIcon(icon);
}

}  // namespace gorgon
