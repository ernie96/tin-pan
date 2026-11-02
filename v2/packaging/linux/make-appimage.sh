#!/bin/sh
# Packages target/release/tinpan (and sunscreen next to it, when the build fetched it) as
# target/release/TinPan.AppImage. GTK 3 and WebKitGTK 4.1 come from the system, as on any desktop
# that runs Orca; bundling WebKitGTK would make the AppImage huge and fragile. Run from v2/ with
# APPIMAGETOOL set to the appimagetool AppImage.
set -eu
release=target/release
appdir="$release/TinPan.AppDir"
here=$(dirname "$0")

rm -rf "$appdir"
mkdir -p "$appdir/usr/bin"
cp "$release/tinpan" "$appdir/usr/bin/tinpan"
if [ -f "$release/sunscreen" ]; then
    cp "$release/sunscreen" "$appdir/usr/bin/sunscreen"
    chmod +x "$appdir/usr/bin/sunscreen"
else
    echo "No sunscreen in $release, so this AppImage can't clean up songs."
fi
cp "$here/tinpan.desktop" "$appdir/tinpan.desktop"
cp "$here/tinpan.svg" "$appdir/tinpan.svg"
ln -s usr/bin/tinpan "$appdir/AppRun"

ARCH=x86_64 "$APPIMAGETOOL" --appimage-extract-and-run "$appdir" "$release/TinPan.AppImage"
echo "Built $release/TinPan.AppImage"
