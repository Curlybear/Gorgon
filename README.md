# Gorgon

A screenshot and upload tool for Linux on Wayland, in the spirit of ShareX.

Capture a region, window or screen, annotate it, upload it to your own server through a ShareX `.sxcu` uploader, shorten the link, and get the URL on your clipboard.

**Status:** early planning and prototyping. See [docs/PLAN.md](docs/PLAN.md) for the plan and [docs/spikes.md](docs/spikes.md) for the M0 test programs.

## Try it

Gorgon is at milestone 2: capture a region, a screen or a window, annotate it in the editor, save it, copy it, get a notification. Uploading comes in milestone 3.

Install the build dependencies listed in [docs/spikes.md](docs/spikes.md#build), then:

```sh
cargo install --path crates/gorgon
```

This puts `gorgon` in `~/.cargo/bin`. Then:

```sh
gorgon capture region          # or screen, window
gorgon capture region --edit   # open the editor before saving
gorgon edit picture.png        # annotate an existing image
```

The first command starts Gorgon in the tray; later ones reuse it. You can also start it yourself with plain `gorgon`.

### Keyboard shortcuts

On KDE, Hyprland and GNOME 48 or newer, Gorgon registers Ctrl+Print (region), Print (screen), Alt+Print (window) and Ctrl+Shift+Print (region, then edit) itself; your desktop asks you to confirm them.

On older GNOME and on Sway, bind the commands yourself. On GNOME: Settings, Keyboard, View and Customize Shortcuts, Custom Shortcuts, then add one per mode with the full path as the command, for example `/home/you/.cargo/bin/gorgon capture region`. GNOME's own screenshot tool owns Print by default, so free that key first under Screenshots if you want it.

### Editor

Opens after `--edit` captures, from the notification's Edit button, or after every capture with `open_editor = true`. Tools have one-key shortcuts: V select, R rectangle, E ellipse, A arrow, L line, P pen, T text, H highlighter, B blur, X pixelate, C crop. Ctrl+Z / Ctrl+Shift+Z undo and redo, Delete removes the selected shape, Enter copies and saves, Esc discards.

Blur and pixelate are baked into the saved pixels, so the original text can't be recovered from the file.

### Settings

Optional, in `~/.config/gorgon/config.toml`:

```toml
save_dir = "~/Pictures/Gorgon"           # default
filename_pattern = "%Y-%m-%d_%H-%M-%S"   # strftime
copy_image = true
notify = true
open_editor = false                      # edit every capture
```

### Logs

When the CLI starts Gorgon, its output goes to `$XDG_RUNTIME_DIR/gorgon/daemon.log`.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
