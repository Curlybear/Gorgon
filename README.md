# Gorgon

A screenshot and upload tool for Linux on Wayland, in the spirit of ShareX.

Capture a region, window or screen, annotate it, upload it to your own server through a ShareX `.sxcu` uploader, shorten the link, and get the URL on your clipboard.

**Status:** early planning and prototyping. See [docs/PLAN.md](docs/PLAN.md) for the plan and [docs/spikes.md](docs/spikes.md) for the M0 test programs.

## Try it

Gorgon is at milestone 1: capture a region, a screen or a window, save it, copy it, get a notification. Uploading comes in milestone 3.

Install the build dependencies listed in [docs/spikes.md](docs/spikes.md#build), then:

```sh
cargo install --path crates/gorgon
```

This puts `gorgon` in `~/.cargo/bin`. Then:

```sh
gorgon capture region    # or screen, window
```

The first command starts Gorgon in the tray; later ones reuse it. You can also start it yourself with plain `gorgon`.

### Keyboard shortcuts

On KDE, Hyprland and GNOME 48 or newer, Gorgon registers Ctrl+Print (region), Print (screen) and Alt+Print (window) itself; your desktop asks you to confirm them.

On older GNOME and on Sway, bind the commands yourself. On GNOME: Settings, Keyboard, View and Customize Shortcuts, Custom Shortcuts, then add one per mode with the full path as the command, for example `/home/you/.cargo/bin/gorgon capture region`. GNOME's own screenshot tool owns Print by default, so free that key first under Screenshots if you want it.

### Settings

Optional, in `~/.config/gorgon/config.toml`:

```toml
save_dir = "~/Pictures/Gorgon"           # default
filename_pattern = "%Y-%m-%d_%H-%M-%S"   # strftime
copy_image = true
notify = true
```

### Logs

When the CLI starts Gorgon, its output goes to `$XDG_RUNTIME_DIR/gorgon/daemon.log`.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
