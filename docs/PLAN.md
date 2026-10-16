# Gorgon: project plan

A Wayland screenshot and upload tool, written in Rust with a Qt (QML) UI. It covers the parts of ShareX that get used daily, not the long tail.

Name: **Gorgon**.

## Decisions so far

| Topic | Decision |
|---|---|
| Display server | Wayland only, through xdg-desktop-portal |
| Desktops | All of them: KDE Plasma, GNOME, Hyprland, Sway and other wlroots compositors |
| Language / UI | Rust + Qt 6 via cxx-qt, UI in QML |
| Audience | Personal first, public later if it turns out good |
| Region selection | Freeze the screen, then select over the frozen image |
| Hotkeys | Global Shortcuts portal, with a CLI command as fallback for any DE |
| Recording | GIF and MP4 of a region, stop by hotkey (second milestone, not MVP) |
| Upload config | ShareX `.sxcu` custom uploader files, for both uploads and URL shorteners |
| Post-capture actions | Configurable in settings |
| App shape | Background app with tray icon, plus a CLI that drives it |
| Editor (v1) | Rectangle, arrow, line, freehand, text, blur/pixelate, highlighter, crop, undo/redo |
| History | List of past captures with thumbnail, local path and URL |
| Packaging | Decided later, once we know what actually works (Flatpak, AppImage, AUR all open) |
| License | GPL-3.0 |
| Repo | New GitHub repo on Curly's account |

## Scope

**In:** region / window / fullscreen screenshots, editor, upload via `.sxcu`, URL shortening via `.sxcu` (Kutt), clipboard, local save, notifications, tray, hotkeys, CLI, history. Then GIF/MP4 recording.

**Out (for now):** OCR, color picker, scrolling capture, screen ruler, image effects, QR codes, image combiner, FTP/SFTP, built-in uploaders for specific services (anything uploadable is reached through `.sxcu`), X11.

## Architecture

```
┌────────────┐   D-Bus    ┌──────────────────────────────────────────┐
│  CLI       │──────────▶│  Daemon (tray app)                        │
│  `gorgon capture region`    │                                          │
└────────────┘            │  hotkeys ─▶ capture ─▶ overlay ─▶ pipeline│
  DE keybinding           │                     (editor)    │         │
  or portal shortcut ────▶│                                 ▼         │
                          │        save · clipboard · upload · shorten│
                          │        · copy URL · notify · history      │
                          └──────────────────────────────────────────┘
```

Cargo workspace:

- **`core`**: config (TOML), after-capture pipeline, `.sxcu` parser and request builder, file naming, history (SQLite via `rusqlite`). No Qt here, so it is fully unit-testable.
- **`capture`**: backends behind a `CaptureBackend` trait. First implementation uses the portal through the `ashpd` crate; a `wlr-screencopy` / `ext-image-copy-capture` backend can be added later for speed on wlroots.
- **`upload`**: `reqwest` + `tokio`, multipart / form / JSON / binary bodies, response parsing.
- **`app`**: cxx-qt + QML. Overlay, editor, settings, history window, tray, the daemon's D-Bus service (`zbus`).
- **`cli`**: thin D-Bus client. `gorgon capture region|window|screen`, `gorgon record region`, `gorgon stop`, `gorgon upload <file>`, `gorgon shorten <url>`. Starts the daemon if it isn't running.

## How each piece works on Wayland

**Screenshot.** Call the Screenshot portal non-interactively to get the whole desktop as one PNG. Most portals ask for permission once and remember it; this needs checking per DE in the spikes.

**Freeze and select.** Show one fullscreen, borderless window per monitor, each drawing its slice of the frozen image, and let the user drag a region (with magnifier, crosshair, and size readout). Use layer-shell (`LayerShellQt`) where available (KDE, wlroots) and plain fullscreen windows on GNOME, which has no layer-shell. Multi-monitor and fractional scaling are the risky part here.

**Window capture.** Wayland doesn't let apps list or locate windows. Two options, to be decided by a spike: the portal's interactive mode (the DE's own picker), or the ScreenCast portal with a "window" source and one grabbed frame. Fullscreen and region are the priority; window mode may ship with whatever the DE offers.

**Hotkeys.** Register through the Global Shortcuts portal (KDE, GNOME 48+, Hyprland). On compositors without it (Sway and some wlroots ones), the settings page shows the CLI commands to bind in the compositor config.

**Clipboard.** Copy the image while the overlay or editor window still has focus, which works everywhere. The URL arrives after the window is gone, and a background app can't always set the clipboard on Wayland: use the data-control protocol where it exists (KDE, wlroots), and on GNOME give the notification a "Copy URL" button. Needs a spike.

**Tray.** StatusNotifierItem (the `ksni` crate, or Qt's tray). Works on KDE and most wlroots bars; GNOME needs the AppIndicator extension, so the CLI and hotkeys must cover everything the tray does.

**Notifications.** `org.freedesktop.Notifications` with actions (open, copy URL, open in browser), through `notify-rust`.

**Recording (milestone 4).** ScreenCast portal + PipeWire, keeping the portal's restore token so you aren't asked every time. Crop to the selected region, encode MP4 with GStreamer or `ffmpeg`, and GIF with `gifski` for quality. A small on-screen border shows the recorded area; the stop hotkey or tray icon ends it.

## Editor

- Annotations live in a Rust model (a list of shapes with properties), so undo/redo is a simple command stack.
- Rendering is pure Rust (tiny-skia for shapes, cosmic-text for text) in `gorgon-core`, so it is unit-tested; the editor shows the same pixels it exports. (cxx-qt-lib's QPainter lacks brushes and painting onto images, which settled it.)
- Blur and pixelate are applied to the pixels on export, never as an overlay, so the hidden text can't be recovered from the uploaded file.
- Tools: select/move, rectangle, arrow, line, freehand, text, highlighter, blur, pixelate, crop. Color, thickness and font size in a small toolbar. Keyboard shortcuts per tool, Enter to finish, Esc to cancel.
- Later, if wanted: numbered step markers, ellipse, shadows.

## Uploads with `.sxcu`

Import `.sxcu` files from a file picker or by drag and drop. XBackBone generates one per user; Kutt needs a small hand-written one (example below). Support:

- `DestinationType`: `ImageUploader`, `FileUploader`, `URLShortener`
- `RequestMethod`, `RequestURL`, `Parameters`, `Headers`
- `Body`: `MultipartFormData`, `FormURLEncoded`, `JSON`, `Binary`, `None`, with `Arguments`, `FileFormName`, `Data`
- Results: `URL`, `ThumbnailURL`, `DeletionURL`, `ErrorMessage`
- Syntax: `{json:path}`, `{regex:...}`, `{response}`, `{header:name}`, `{input}`, `{filename}`, `{random:...}`. Anything else is reported as unsupported on import, not silently ignored.

Settings pick one default image uploader and one default URL shortener. Files that contain API tokens are stored in the config folder with owner-only permissions; Secret Service storage can come later.

Example Kutt shortener:

```json
{
  "Version": "16.0.0",
  "Name": "Kutt",
  "DestinationType": "URLShortener",
  "RequestMethod": "POST",
  "RequestURL": "https://kutt.example.com/api/v2/links",
  "Headers": { "X-API-KEY": "<your key>" },
  "Body": "JSON",
  "Data": "{\"target\":\"{input}\"}",
  "URL": "{json:link}"
}
```

## After-capture pipeline

An ordered list of steps, each toggled in settings (defaults marked):

1. Open editor (off, can be forced per hotkey, e.g. a separate "capture and edit" shortcut)
2. Copy image to clipboard (on)
3. Save to disk, with a filename pattern like `%Y-%m-%d_%H-%M-%S` (on)
4. Upload with the default uploader (on)
5. Shorten the URL with the default shortener (on)
6. Copy the final URL to clipboard (on)
7. Show a notification with actions (on)

Every capture and its result goes into history, failed uploads included, with a "retry upload" action.

## Milestones

**M0: Spikes** (answers the risky questions before building on them)
- Non-interactive portal screenshot on KDE, GNOME, Hyprland, Sway: does it prompt, how fast is it, what scale does the image come at?
- Fullscreen frozen overlay on each, with two monitors at different scales
- cxx-qt app with a tray icon and a `QQuickPaintedItem` written in Rust
- Global Shortcuts portal registration on KDE, GNOME, Hyprland
- Setting the clipboard from a background app on each DE
- Window capture: portal interactive vs ScreenCast single frame

**M1: Capture** (done): daemon, tray, CLI over D-Bus, region / fullscreen / window capture, freeze overlay, save to disk, copy image, notification, hotkeys.

**M2: Editor** (first version on main): all v1 tools, undo/redo, export, "capture and edit" path.

**M3: Upload** (done: `.sxcu` uploads, shortening, URL copy, CLI, History and Settings windows): `.sxcu` import and execution, XBackBone + Kutt working end to end, URL copy, history window, settings UI for the pipeline. **This is the MVP: daily-usable.**

**M4: Recording** (done, 2026-10-16): region GIF and MP4 through the ScreenCast portal and GStreamer, gifski for GIFs, stop hotkey (same shortcut again, `gorgon record stop`, or the notification), tray icon and notification as the indicator, same upload pipeline and history. An on-screen border around the recorded area was skipped: GNOME doesn't let apps position windows.

**M5: Going public**: packaging (whichever works best by then), README with screenshots, CI building releases, sample `.sxcu` files.

## Risks

- **Overlay behavior differs per compositor** (layer-shell missing on GNOME, fractional scaling, multi-monitor). Mitigated by the M0 spike and by keeping the overlay code isolated.
- **Portal permission prompts** could make capture feel slow or naggy on some DEs. Fallback: native backends (wlr-screencopy on wlroots).
- **cxx-qt maturity**: it is usable but younger than plain C++ Qt; keep Qt-specific code thin and logic in plain Rust crates.
- **Clipboard from the background** on GNOME, handled with the notification button.
- **Global Shortcuts portal** missing on some compositors, covered by the CLI.

## Testing

- Unit tests in `core` and `upload`: `.sxcu` parsing against real files (XBackBone, Kutt, a few public ones), request building, response parsing, filename patterns.
- HTTP tests against a mock server (`wiremock`).
- Editor rendering snapshot tests: render a known annotation list to an image and compare.
- Manual test matrix per milestone across KDE, GNOME, Hyprland, Sway (VMs or nested sessions).
