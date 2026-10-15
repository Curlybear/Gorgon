// Small bits of Qt that cxx-qt-lib doesn't expose.
#pragma once

#include <cstdint>

#include "rust/cxx.h"

namespace gorgon {

// Puts an RGBA8 image on the clipboard. On Wayland this only works while one
// of Gorgon's windows has keyboard focus.
bool set_clipboard_image(rust::Slice<const std::uint8_t> rgba, std::int32_t width,
                         std::int32_t height);

// Keeps the app running in the tray after the last overlay closes.
void keep_running_without_windows();

}  // namespace gorgon
