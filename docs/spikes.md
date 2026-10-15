# M0 spikes

Small throwaway programs that answer the risky questions in [PLAN.md](PLAN.md#milestones) before real code depends on them. Run them on each desktop you care about and paste the output into the results section below (or an issue).

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

The first build takes a few minutes. The programs end up in `target/release/`.

## What to run

### 1. Session report

```sh
./target/release/gorgon-probe
```

Lists the Wayland protocols and portal versions Gorgon cares about (layer-shell, data-control, screencopy, Screenshot / ScreenCast / GlobalShortcuts portals, native screenshot targets).

### 2. Portal screenshot

```sh
./target/release/gorgon-probe screenshot --runs 3
./target/release/gorgon-probe screenshot --interactive --runs 1
./target/release/gorgon-probe screenshot --target window --runs 1   # only if the report lists Window
```

Note how long each run takes, the image size, and whether a permission dialog appeared (and on which runs).

### 3. Global shortcuts

```sh
./target/release/gorgon-probe shortcuts --seconds 60
```

Note whether a dialog asked to confirm the shortcuts, what triggers were bound, and whether pressing them prints `activated`. Run it a second time to see if the binding is remembered.

### 4. Clipboard without a window

```sh
./target/release/gorgon-probe clipboard
```

While it waits, paste somewhere. Then copy something else; the probe exits. If it fails immediately, the daemon needs the notification "Copy URL" fallback on this desktop.

### 5. Frozen overlay

```sh
./target/release/gorgon-overlay-spike
./target/release/gorgon-overlay-spike --tray     # starts in the tray; click it or use its menu
```

Takes a screenshot, then covers every monitor with it. Drag a region; it's saved to `/tmp/gorgon-spike-*.png`. Esc cancels. Check:

- Does the frozen image line up exactly with what was on screen, on every monitor?
- Mixed scales (for example 100% and 150%): is it sharp, and is the saved crop exactly what you selected?
- Does the overlay cover panels and docks, or do they stay on top?
- Does Esc work straight away (keyboard focus)?
- With `--tray`: does an icon appear, and does clicking it (or its menu) start a capture? If it prints `tray failed: ... StatusNotifierWatcher`, your desktop has no tray host: on GNOME that means the AppIndicator extension is off (Ubuntu ships it on by default).

Set `GORGON_SPIKE_IMAGE=/path/to.png` to skip the portal and use a fixed image.

## Results

Copy this block per desktop.

```
Desktop / version:
Monitors and scales:

report:        (paste)
screenshot:    time per run =     prompt on runs =     size matches monitors? y/n
interactive:   works y/n, notes
target window: works y/n / not offered
shortcuts:     dialog y/n, bound triggers =     activations work y/n, remembered y/n
clipboard:     works y/n / error
overlay:       lines up y/n, sharp y/n, crop correct y/n, covers panels y/n, Esc y/n
tray:          icon y/n
```
