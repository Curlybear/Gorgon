#!/bin/sh
# Builds Gorgon-<version>-x86_64.AppImage in target/appimage.
# Needs linuxdeploy and linuxdeploy-plugin-qt on PATH (or LINUXDEPLOY set),
# Qt 6 with qmake6, and a release build (cargo build --release -p gorgon).
# GStreamer is not bundled: recording uses the system's gst-launch-1.0.
set -eu

cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/gorgon/Cargo.toml | head -n1)
out=target/appimage
appdir=$out/AppDir
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/metainfo"

install -m755 target/release/gorgon "$appdir/usr/bin/gorgon"
for f in assets/icons/gorgon-*.png; do
    size=${f##*-}; size=${size%.png}
    install -Dm644 "$f" "$appdir/usr/share/icons/hicolor/${size}x${size}/apps/gorgon.png"
done
install -m644 packaging/io.github.curlybear.gorgon.metainfo.xml "$appdir/usr/share/metainfo/"

export QMAKE="${QMAKE:-qmake6}"
export QML_SOURCES_PATHS=crates/gorgon/qml
export EXTRA_PLATFORM_PLUGINS="libqwayland-generic.so"
export EXTRA_QT_MODULES="waylandclient"
export LDAI_OUTPUT="$out/Gorgon-$version-x86_64.AppImage"
export APPIMAGE_EXTRACT_AND_RUN=1

linuxdeploy="${LINUXDEPLOY:-linuxdeploy}"
"$linuxdeploy" --appdir "$appdir" \
    --desktop-file packaging/gorgon.desktop \
    --icon-file assets/icons/gorgon-256.png --icon-filename gorgon \
    --plugin qt

# The Qt plugin leaves out the Wayland EGL client integration, without which
# Qt Quick falls back to software rendering on Wayland.
plugins=$("$QMAKE" -query QT_INSTALL_PLUGINS)
mkdir -p "$appdir/usr/plugins/wayland-graphics-integration-client"
cp "$plugins"/wayland-graphics-integration-client/*.so \
    "$appdir/usr/plugins/wayland-graphics-integration-client/"
"$linuxdeploy" --appdir "$appdir" --output appimage \
    $(for f in "$appdir"/usr/plugins/wayland-graphics-integration-client/*.so; do
        printf -- '--deploy-deps-only %s ' "$f"; done)

echo "Built $LDAI_OUTPUT"
