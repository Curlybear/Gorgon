# Going public: checklist

M1 to M4 are done and verified on GNOME 50. This is everything between here and a public first release (M5).

## Questions to decide first

Each one changes what the checklist below needs. My recommendation is marked.

1. **Packaging: what ships first?**
   - Flatpak on Flathub (**recommended**): one build for every distro, the portals Gorgon depends on are native there, and GStreamer and Qt come with the runtime.
   - AUR package: cheap to add next to the Flatpak, and Arch users expect one.
   - AppImage: works, but bundling Qt 6 and GStreamer makes it large and fiddly.
   - `.deb`/PPA: only helps Ubuntu and Debian.
2. **gifski's AGPL-3.0 license: keep it or swap it?** Keeping it is legal, but anyone redistributing Gorgon binaries takes on AGPL terms for gifski. The alternative is the MIT `gif` crate, which gives worse GIFs.
   - Keep gifski (**recommended**).
   - Switch to an MIT encoder.
3. **Which desktops do we claim?** So far it has only been tested on GNOME 50.
   - "Tested on GNOME and KDE, others best effort" (**recommended**, after one KDE pass).
   - Test Hyprland and Sway too before release.
4. **Commit authorship.** Every commit is authored by "Claude &lt;noreply@anthropic.com&gt;". The repo will look like a bot wrote it.
   - Keep the history as it is (**recommended**, honest and zero risk).
   - Rewrite authorship to you before going public. This needs a force push to main, and it can only be done before anyone has cloned the repo.
5. **The icon.** It was generated with Gemini.
   - Fine to publish as is (**recommended**, but check you're happy with Google's terms for generated images).
   - Commission or draw a simplified version. At tray size (22 px) the detail turns into a blob.
6. **First version number.**
   - 0.1.0 (**recommended**).
   - 1.0.0.

## Before the repo goes public

- [ ] Rewrite the README opening. It still says "early planning and prototyping".
- [ ] Add a screenshot of the editor and a short GIF of a recording to the README. Gorgon can make both.
- [ ] Add a "Supported desktops" table and a troubleshooting section to the README: the AppIndicator extension on GNOME, shortcut conflicts, and missing GStreamer plugins.
- [ ] Check that nothing private is in the git history. A scan of the current tree found no tokens or personal server URLs. The test files only hold placeholders like `YOUR_TOKEN`.
- [ ] Decide what stays in the repo:
  - `spikes/` and `docs/spikes.md` could move to a branch, or be kept as history.
  - `tools/fake-portal` is a test tool, so keep it under `tools/` and document it.
- [ ] Add a CONTRIBUTING.md (how to build, run the tests, and do the headless test setup) and issue templates for bugs, with the desktop, versions and logs.
- [ ] Add `repository`, `homepage` and `keywords` to the crates' Cargo.toml files.

## Code that should change for packaging

- [ ] Stop self-installing the icon and launcher entry when a package already provides them. Today Gorgon writes into `~/.local/share` at every start. A package ships them in `/usr/share`, or the Flatpak export does.
- [ ] Ship the launcher entry, icons and an AppStream metainfo file (`<app id>.metainfo.xml`) in the repo. Flathub and the software centers require the metainfo file.
- [ ] Pick a reverse-DNS app id for Flatpak, for example `io.github.curlybear.Gorgon`. The D-Bus name, the desktop file and the shortcut ids follow from it, so changing the app id resets people's approved shortcuts once.
- [ ] Add `gorgon uninstall-user-files`, or document how to remove the autostart entry, launcher, icons, config, history and screencast token.
- [ ] Make `tools/xbackbone-raw-links` unnecessary by letting Settings edit an uploader's URL template, or by offering "raw link" as an XBackBone option on import.

## CI and releases

- [ ] A GitHub Actions workflow on every push: `cargo fmt --check`, `cargo clippy`, `cargo test` (the core tests need GStreamer for the recording tests), and a release build.
- [ ] Tag-triggered releases that build the chosen package(s) and attach them to a GitHub release.
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
