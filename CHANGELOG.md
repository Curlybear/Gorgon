# Changelog

## Unreleased

- Security: uploads no longer follow redirects to another host (which would carry API keys along), links from the server open only when they are `http`/`https`, server text is shown as plain text, and history, settings and the screen-sharing token are saved readable only by you. Importing an `http://` uploader warns that it is unencrypted.
- Releases are built without write access, use pinned and checksummed linuxdeploy, and come with `SHA256SUMS` and a build attestation. See SECURITY.md to report a problem.
- An unknown key in config.toml is now ignored with a warning instead of dropping every setting.
- Captures no longer fail silently on GNOME when Gorgon was started at login without screenshot permission: Gorgon falls back to GNOME's screenshot dialog, and the README explains how to allow silent captures.
- Captures whose path contains `#` or `%` open in the editor and History again.

## 0.1.0 (2026-10-17)

The first release.

- Region (on a frozen screen), window and screen captures through xdg-desktop-portal, from global shortcuts, the tray or the `gorgon` command.
- Editor with rectangle, ellipse, arrow, line, pen, text, highlighter, blur, pixelate and crop, undo/redo and zoom. Blur and pixelate are baked into the saved pixels.
- MP4 and GIF region recording through the ScreenCast portal and GStreamer, with gifski for GIFs. Ctrl+Alt+Print and Ctrl+Alt+Shift+Print start and stop it.
- Uploads and short links through ShareX `.sxcu` files, tested with XBackBone and Kutt, and `gorgon uploader raw-links` for XBackBone links straight to the file.
- History and Settings windows, notifications with actions, and the image or link copied to the clipboard.
- Start at login, an app launcher entry and the Gorgon icon.
- `.deb` and AppImage packages.

Tested on GNOME 50. Other desktops are still to be tested.
