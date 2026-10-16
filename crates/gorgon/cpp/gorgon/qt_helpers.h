// Small bits of Qt that cxx-qt-lib doesn't expose.
#pragma once

#include <cstdint>

#include "rust/cxx.h"

namespace gorgon {

// Puts an RGBA8 image on the clipboard. On Wayland this only works while one
// of Gorgon's windows has keyboard focus.
bool set_clipboard_image(rust::Slice<const std::uint8_t> rgba, std::int32_t width,
                         std::int32_t height);

// Puts text on the clipboard; same focus rule as images.
void set_clipboard_text(rust::Str text);

// Keeps the app running in the tray after the last overlay closes.
void keep_running_without_windows();

// Sets the desktop entry name (the Wayland app id) and the window icon.
void set_app_identity(rust::Str desktop_file, rust::Str icon_path);

}  // namespace gorgon
