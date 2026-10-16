<p align="center"><img src="assets/icons/gorgon-256.png" alt="Gorgon icon" width="160"></p>

# Gorgon

A screenshot and upload tool for Linux on Wayland, in the spirit of ShareX.

Capture a region, window or screen, annotate it, upload it to your own server through a ShareX `.sxcu` uploader, shorten the link, and get the URL on your clipboard.

**Status:** early planning and prototyping. See [docs/PLAN.md](docs/PLAN.md) for the plan and [docs/spikes.md](docs/spikes.md) for the M0 test programs.

## Try it

Gorgon is at milestone 4: capture a region, a screen or a window, annotate it in the editor, record a region to MP4 or GIF, upload it to your server and copy the link.

Install the build dependencies listed in [docs/spikes.md](docs/spikes.md#build), then:

```sh
cargo install --path crates/gorgon
```

This puts `gorgon` in `~/.cargo/bin`. Then:

```sh
gorgon capture region          # or screen, window
gorgon capture region --edit   # open the editor before saving
gorgon edit picture.png        # annotate an existing image
gorgon record mp4              # or gif; drag a region, then...
gorgon record stop             # ...stop (or run the same record command again)
```

The first command starts Gorgon in the tray; later ones reuse it. Once it has run, Gorgon also appears in your app grid (it opens History, with Capture region and Settings on right-click). You can also start it yourself with plain `gorgon`. To have it start when you log in, tick "Start Gorgon when I log in" in Settings or run `gorgon autostart on` (`off` to undo); it adds `~/.config/autostart/gorgon.desktop`, which GNOME, KDE and most desktops run at login. On Hyprland or Sway, add `exec-once = gorgon` / `exec gorgon` to your compositor config instead.

### Supported desktops

Gorgon talks to the desktop through xdg-desktop-portal, so it should work anywhere the portals do. So far:

| Desktop | Status | Notes |
|---|---|---|
| GNOME 50 (Ubuntu 26.04) | Tested | Tray needs the AppIndicator extension |
| GNOME 46–49 | To be tested | Global shortcuts need GNOME 48 or newer |
| KDE Plasma 6 | To be tested | |
| Hyprland | To be tested | |
| Sway and other wlroots compositors | To be tested | No global shortcuts portal: bind the `gorgon` commands in your config |

Tried it somewhere else? Open an issue with what worked and what didn't.

### Keyboard shortcuts

On KDE, Hyprland and GNOME 48 or newer, Gorgon registers Ctrl+Print (region), Print (screen), Alt+Print (window) and Ctrl+Shift+Print (region, then edit), Ctrl+Alt+Print (record a region to MP4) and Ctrl+Alt+Shift+Print (record a region to GIF) itself; press a record shortcut again to stop; your desktop asks you to confirm them.

On older GNOME and on Sway, bind the commands yourself. On GNOME: Settings, Keyboard, View and Customize Shortcuts, Custom Shortcuts, then add one per mode with the full path as the command, for example `/home/you/.cargo/bin/gorgon capture region`. GNOME's own screenshot tool owns Print by default, so free that key first under Screenshots if you want it.

### Editor

Opens after `--edit` captures, from the notification's Edit button, or after every capture with `open_editor = true`. Tools have one-key shortcuts: V select, R rectangle, E ellipse, A arrow, L line, P pen, T text, H highlighter, B blur, X pixelate, C crop. Ctrl+Z / Ctrl+Shift+Z undo and redo, Delete removes the selected shape, Enter copies and saves, Esc discards. The bar at the bottom shows the zoom; type a percentage there, or use Ctrl+wheel, Ctrl+plus/minus, Ctrl+0 to fit and Ctrl+1 for actual pixels. Scroll, drag empty space with the Select tool, middle-drag or Space+drag to move around a zoomed-in capture.

Blur and pixelate are baked into the saved pixels, so the original text can't be recovered from the file.

### Recording

Ctrl+Alt+Print, Ctrl+Alt+Shift+Print, the tray menu or `gorgon record mp4|gif` freeze the screen so you can drag the area to record. The first time, your desktop asks which screen to share; Gorgon remembers the answer. While recording, the tray icon turns red and a notification offers Stop. The file lands in `save_dir` and goes through the same upload and history as captures.

Recording needs GStreamer at runtime:

```sh
sudo apt install gstreamer1.0-tools gstreamer1.0-pipewire gstreamer1.0-plugins-good gstreamer1.0-plugins-ugly
```

Without the "ugly" plugins (x264), Gorgon falls back to OpenH264 or WebM. GIFs are encoded with gifski.

### History and settings

`gorgon history` (or History in the tray menu) lists past captures with
thumbnails and links: copy or open a link, upload again after a failure,
edit, show in folder, or delete. It lives in
`~/.local/share/gorgon/history.json`.

`gorgon settings` (or Settings in the tray menu) edits everything below and
imports, picks and removes uploaders (drop `.sxcu` files on it).

### Settings

In `~/.config/gorgon/config.toml`, also editable from the Settings window:

```toml
save_dir = "~/Pictures/Gorgon"           # default
filename_pattern = "%Y-%m-%d_%H-%M-%S"   # strftime
copy_image = true
notify = true
open_editor = false                      # edit every capture
upload = true                            # when image_uploader is set
image_uploader = "XBackBone"             # set by `gorgon uploader import`
shorten = true                           # when url_shortener is set
url_shortener = "Kutt"
copy_url = true                          # the link replaces the copied image
video_fps = 30                           # MP4 recordings
gif_fps = 15                             # GIF recordings
```

### Uploading and short links

Gorgon uploads through ShareX custom uploader files (`.sxcu`), so any server
that ShareX can upload to works. Import one per destination:

```sh
gorgon uploader import ~/Downloads/xbackbone.sxcu   # from XBackBone's ShareX config download
gorgon uploader import kutt.sxcu                    # see examples/kutt.sxcu
gorgon uploader list
```

The first uploader and the first URL shortener you import become the
defaults; `gorgon uploader default <name>` switches. Imported files live in
`~/.config/gorgon/uploaders/`, readable only by you, since they hold your
tokens. After each capture Gorgon saves it, uploads it, shortens the link,
copies the link and shows it in a notification (with Open link and Copy link
buttons). `examples/` has templates for XBackBone and Kutt.

From the command line:

```sh
gorgon upload shot.png          # prints the (short) link
gorgon upload shot.png --no-shorten --with XBackBone
gorgon shorten https://example.com/long/link
```

Supported `.sxcu` features: all request methods, `Parameters`, `Headers`,
`MultipartFormData` / `FormURLEncoded` / `JSON` / `XML` / `Binary` bodies,
`URL`, `ThumbnailURL`, `DeletionURL`, `ErrorMessage`, and `{json:...}`,
`{regex:...|group}`, `{header:...}`, `{response}`, `{responseurl}`,
`{filename}`, `{input}`, `{random:a|b}`, `{base64:...}`. Older files using
`$json:url$` and `RegexList` work too. Files that need `{xml:...}` or
interactive prompts are refused on import, with the reason.

### Logs

When the CLI starts Gorgon, its output goes to `$XDG_RUNTIME_DIR/gorgon/daemon.log`.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE). Builds link gifski, which is AGPL-3.0-or-later; GPLv3 section 13 allows that combination, and the AGPL terms then apply to gifski within the binary.
