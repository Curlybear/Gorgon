# Packaging

Pushing a tag `vX.Y.Z` (matching the version in `crates/gorgon/Cargo.toml`)
makes `.github/workflows/release.yml` build both packages on Ubuntu 24.04 and
attach them to a GitHub release.

To build them by hand:

```sh
cargo build --release -p gorgon

# .deb (Debian 13, Ubuntu 24.04 and newer)
cargo install cargo-deb
cargo deb -p gorgon --no-build          # → target/debian/

# AppImage: put linuxdeploy and linuxdeploy-plugin-qt on PATH first
packaging/build-appimage.sh             # → target/appimage/
```

The AppImage bundles Qt but not GStreamer, so recording needs the system's
`gst-launch-1.0` and plugins, the same as with the .deb. Both packages ship
`gorgon.desktop`, the icons and `io.github.curlybear.gorgon.metainfo.xml`
from this folder and `assets/icons`. A `cargo install` or AppImage copy
installs the launcher and icons into `~/.local/share` itself on start; a
package under `/usr` doesn't need to.
