# Contributing to Gorgon

Bug reports, testing on desktops we haven't tried, and patches are all welcome. For anything bigger than a fix, open an issue first so we can agree on the approach.

## Build

You need a Wayland session with xdg-desktop-portal running (any modern KDE, GNOME, Hyprland or Sway setup has it), plus a C++ compiler, Qt 6 with Qt Quick, and Rust.

### 1. Install system packages

Arch / Manjaro / EndeavourOS:

```sh
sudo pacman -S --needed base-devel git rustup qt6-base qt6-declarative
```

Fedora:

```sh
sudo dnf install gcc-c++ git curl qt6-qtbase-devel qt6-qtdeclarative-devel
```

Debian 13 / Ubuntu 24.04 or newer:

```sh
sudo apt install build-essential git curl qt6-base-dev qt6-declarative-dev \
  qml6-module-qtquick qml6-module-qtquick-window qml6-module-qtqml-workerscript
```

### 2. Install Rust

On Arch, `rustup` came from pacman above. Elsewhere, install it from rustup.rs:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # skip on Arch
rustup default stable
```

Open a new terminal afterwards (or `source ~/.cargo/env`) so `cargo` is on your PATH.

### 3. Point the build at Qt 6's qmake

The build finds Qt through `qmake`. If `qmake --version` already says Qt 6, skip this. Otherwise:

```sh
export QMAKE=qmake6                      # Arch, Fedora
export QMAKE=/usr/lib/qt6/bin/qmake      # Debian / Ubuntu
```

### 4. Build

```sh
git clone https://github.com/Curlybear/Gorgon.git
cd Gorgon
cargo build --release
```

The first build takes a few minutes. The programs end up in `target/release/`. For recording, also install the GStreamer packages listed in the README.

## Layout

- `crates/gorgon-core`: everything that doesn't need Qt (config, capture through the portals, the `.sxcu` engine, uploads, history, recording, the editor's rendering). Most tests live here.
- `crates/gorgon`: the tray app, CLI, D-Bus service and QML windows (cxx-qt).
- `tools/fake-portal`: a stand-in Screenshot portal for headless runs (below).
- `spikes/`: the M0 test programs that checked each desktop's portals before Gorgon was built. `gorgon-probe` is still handy for reporting what a desktop supports.
- `packaging/`: desktop file, AppStream metainfo and the AppImage script; see `packaging/README.md`.

## Tests

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

The recording tests use GStreamer's `videotestsrc`, so they need `gstreamer1.0-tools` and the base and good plugins, but no screen. CI runs the same commands on every push.

## Running without a desktop

Gorgon can run under Xvfb with `tools/fake-portal` standing in for the Screenshot portal, which is how the UI was tested in containers:

```sh
Xvfb :99 -screen 0 1600x1000x24 &
export DISPLAY=:99 QT_QPA_PLATFORM=xcb
dbus-run-session -- sh -c '
  target/release/fake-portal some-screenshot.png &
  target/release/gorgon daemon &
  sleep 3
  target/release/gorgon capture region --edit
  sleep 2
  xdotool mousemove 300 200 mousedown 1 mousemove 700 500 mouseup 1
  sleep 2
  import -window root window.png    # ImageMagick
'
```

Set `GORGON_TEST_RECORD=1` to make recording use a test pattern instead of the ScreenCast portal.

## Commits

Small, focused commits with a message that says what changed and why. Run `cargo fmt` before committing.
