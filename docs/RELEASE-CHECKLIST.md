# Going public: checklist

M1 to M4 are done and verified on GNOME 50. This is everything between here and a public first release (M5).

## Decisions (Curly, 2026-10-16)

1. **Packaging:** AppImage and `.deb`, both built by CI and attached to GitHub releases.
2. **GIF encoder:** keep gifski (AGPL-3.0, noted in the README).
3. **Desktops:** a README table, "tested" for GNOME and "to be tested" for the rest, updated as each gets tested.
4. **Commit authorship:** commits are co-authored by Curly and Claude from now on (`Co-authored-by` trailers). Rewriting the existing history to match needs a force push and is a separate yes/no.
5. **Icon:** ship it as is.
6. **First version:** 0.1.0.

## Before the repo goes public

- [ ] Rewrite the README opening. It still says "early planning and prototyping".
- [ ] Add a screenshot of the editor and a short GIF of a recording to the README. Gorgon can make both.
- [x] Add a "Supported desktops" table.
- [ ] Add a troubleshooting section to the README: the AppIndicator extension on GNOME, shortcut conflicts, and missing GStreamer plugins.
- [ ] Check that nothing private is in the git history. A scan of the current tree found no tokens or personal server URLs. The test files only hold placeholders like `YOUR_TOKEN`.
- [ ] Decide what stays in the repo:
  - `spikes/` and `docs/spikes.md` could move to a branch, or be kept as history.
  - `tools/fake-portal` is a test tool, so keep it under `tools/` and document it.
- [ ] Add a CONTRIBUTING.md (how to build, run the tests, and do the headless test setup) and issue templates for bugs, with the desktop, versions and logs.
- [ ] Add `repository`, `homepage` and `keywords` to the crates' Cargo.toml files.

## Code that should change for packaging

- [x] Stop self-installing the icon and launcher entry when a package already provides them. Today Gorgon writes into `~/.local/share` at every start. A package ships them in `/usr/share`, or the Flatpak export does.
- [x] Ship the launcher entry, icons and an AppStream metainfo file (`<app id>.metainfo.xml`) in the repo. Flathub and the software centers require the metainfo file.
- [x] Pick a reverse-DNS app id: `io.github.curlybear.gorgon` for AppStream; the desktop file stays `gorgon.desktop` so approved shortcuts survive. ~~Pick a reverse-DNS app id (AppStream wants one even without Flatpak), for example `io.github.curlybear.Gorgon`. The D-Bus name, the desktop file and the shortcut ids follow from it, so changing the app id resets people's approved shortcuts once.~~
- [ ] Add `gorgon uninstall-user-files`, or document how to remove the autostart entry, launcher, icons, config, history and screencast token.
- [ ] Make `tools/xbackbone-raw-links` unnecessary by letting Settings edit an uploader's URL template, or by offering "raw link" as an XBackBone option on import.

## CI and releases

- [x] A GitHub Actions workflow on every push: `cargo fmt --check`, `cargo clippy`, `cargo test` (the core tests need GStreamer for the recording tests), and a release build.
- [x] Tag-triggered releases that build the chosen package(s) and attach them to a GitHub release.
- [ ] A CHANGELOG.md, starting with 0.1.0.
- [ ] Run a dependency license check (`cargo deny check licenses`) to confirm nothing besides gifski needs a note.

## Testing before release

- [ ] KDE Plasma 6: region, screen, window, shortcuts, tray, clipboard, recording.
- [ ] Two monitors at different scales on GNOME and on KDE. Neither has been tried yet.
- [ ] Hyprland and Sway, if we claim them: shortcuts come from the CLI there, and recording's region offset needs checking.
- [ ] A fresh-user run: no config, no uploaders, first-run portal prompts. Does everything explain itself?
- [ ] A packaged build, not `cargo install`, on a clean machine or VM.

## Nice to have, not blocking

- The `video_fps` and `gif_fps` settings in the Settings window (today they're only in config.toml).
- A border around the recorded area on desktops with layer-shell (KDE, wlroots). GNOME can't do it.
- Editor extras the plan listed as "later": numbered step markers and shadows.
- Storing uploader tokens in the Secret Service instead of owner-only files.
- A simplified icon for small sizes.
