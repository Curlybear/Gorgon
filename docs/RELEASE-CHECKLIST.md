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

- [x] Rewrite the README opening. It still says "early planning and prototyping".
- [x] Add a screenshot of the editor to the README (no demo GIF, Curly's call).
- [x] Add a "Supported desktops" table.
- [x] Add a troubleshooting section to the README: the AppIndicator extension on GNOME, shortcut conflicts, and missing GStreamer plugins.
- [x] Check that nothing private is in the git history: a scan of every commit found no tokens, personal server URLs or email addresses.
- [x] Decide what stays in the repo: both stay, described in CONTRIBUTING.md.
  - `spikes/` and `docs/spikes.md` could move to a branch, or be kept as history.
  - `tools/fake-portal` is a test tool, so keep it under `tools/` and document it.
- [x] Add a CONTRIBUTING.md (how to build, run the tests, and do the headless test setup) and issue templates for bugs, with the desktop, versions and logs.
- [x] Add `repository`, `homepage` and `keywords` to the crates' Cargo.toml files.

## Code that should change for packaging

- [x] Stop self-installing the icon and launcher entry when a package already provides them. Today Gorgon writes into `~/.local/share` at every start. A package ships them in `/usr/share`, or the Flatpak export does.
- [x] Ship the launcher entry, icons and an AppStream metainfo file (`<app id>.metainfo.xml`) in the repo. Flathub and the software centers require the metainfo file.
- [x] Pick a reverse-DNS app id: `io.github.curlybear.gorgon` for AppStream; the desktop file stays `gorgon.desktop` so approved shortcuts survive. ~~Pick a reverse-DNS app id (AppStream wants one even without Flatpak), for example `io.github.curlybear.Gorgon`. The D-Bus name, the desktop file and the shortcut ids follow from it, so changing the app id resets people's approved shortcuts once.~~
- [x] Document (README, Uninstall) how to remove the autostart entry, launcher, icons, config, history and screencast token.
- [x] Replace `tools/xbackbone-raw-links` with `gorgon uploader raw-links <name>`.

## CI and releases

- [x] A GitHub Actions workflow on every push: `cargo fmt --check`, `cargo clippy`, `cargo test` (the core tests need GStreamer for the recording tests), and a release build.
- [x] Tag-triggered releases that build the chosen package(s) and attach them to a GitHub release.
- [x] A CHANGELOG.md, starting with 0.1.0.
- [x] Run a dependency license check: `cargo deny check licenses` passes (deny.toml), gifski is the only copyleft dependency, and CI runs the check.

## Testing before release

- [x] A fresh-user run (no config, no uploaders): the capture saves, History shows it as not uploaded, and `gorgon upload` says how to import an uploader.
- [x] The packaged builds: the .deb installed and ran, and the AppImage started its daemon and windows.
- Other desktops stay "to be tested" in the README table until someone tries them.

## Nice to have, not blocking

- The `video_fps` and `gif_fps` settings in the Settings window (today they're only in config.toml).
- A border around the recorded area on desktops with layer-shell (KDE, wlroots). GNOME can't do it.
- Editor extras the plan listed as "later": numbered step markers and shadows.
- Storing uploader tokens in the Secret Service instead of owner-only files.
- A simplified icon for small sizes.
