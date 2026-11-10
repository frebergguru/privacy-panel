#!/bin/bash
# Runs inside an archlinux container, started by tools/build-appimage.sh. Do not run it
# on your own machine: it installs packages and writes to /usr.
#
#   /src    the repository (read only)
#   /build  output: cargo target dir, AppDir and dist/
set -euo pipefail

QUICK_SHARUN_COMMIT=ce344325ab1db718a7ad4ce9868a92a38d4af12a
QUICK_SHARUN_SHA256=0329d9d6742d741a8cecf1c09d825016c8c5d911b316c3bdabf0cca993fd5b10

: "${VERSION:?}" "${HOST_UID:?}" "${HOST_GID:?}"

pacman -Syu --noconfirm --needed \
    base-devel rustup gtk4 libadwaita git curl \
    xorg-server-xvfb strace patchelf binutils zsync \
    adwaita-icon-theme hicolor-icon-theme
rustup default stable >/dev/null
rustup target add x86_64-unknown-linux-musl >/dev/null

export CARGO_TARGET_DIR=/build/target
cd /src
# The app links the GTK stack and is bundled with it below. The helper runs as root
# outside the AppImage (see crates/gui/src/pkexec.rs), so it is a static musl binary
# that needs nothing from the host.
cargo build --release --locked -p privacy-panel
cargo build --release --locked -p privacy-panel-helper --target x86_64-unknown-linux-musl
HELPER=$CARGO_TARGET_DIR/x86_64-unknown-linux-musl/release/privacy-panel-helper
if readelf -d "$HELPER" | grep -q NEEDED; then
    echo "error: $HELPER is not static" >&2
    exit 1
fi

# quick-sharun deploys what is installed under /usr.
install -Dm755 "$CARGO_TARGET_DIR/release/privacy-panel" /usr/bin/privacy-panel
install -Dm644 data/guru.freberg.PrivacyPanel.desktop /usr/share/applications/guru.freberg.PrivacyPanel.desktop
install -Dm644 data/icons/guru.freberg.PrivacyPanel.svg /usr/share/icons/hicolor/scalable/apps/guru.freberg.PrivacyPanel.svg
install -Dm644 data/guru.freberg.PrivacyPanel.metainfo.xml /usr/share/metainfo/guru.freberg.PrivacyPanel.metainfo.xml

mkdir -p /opt/qs
curl -fsSL -A 'Mozilla/5.0' -o /opt/qs/quick-sharun \
    "https://raw.githubusercontent.com/pkgforge-dev/Anylinux-AppImages/$QUICK_SHARUN_COMMIT/useful-tools/quick-sharun.sh"
echo "$QUICK_SHARUN_SHA256  /opt/qs/quick-sharun" | sha256sum -c -
chmod +x /opt/qs/quick-sharun

mkdir -p /build/work
cd /build/work
export ARCH=x86_64
export APPDIR=/build/work/AppDir
export OUTPATH=/build/dist
export DESKTOP=/usr/share/applications/guru.freberg.PrivacyPanel.desktop
export ICON=/usr/share/icons/hicolor/scalable/apps/guru.freberg.PrivacyPanel.svg
export DEPLOY_GTK=1
# GPU drivers come from the host; GTK falls back to software drawing without them.
export USE_HOST_DRIVERS_EXPERIMENTAL=1
export UPINFO="gh-releases-zsync|frebergguru|privacy-panel|latest|*x86_64.AppImage.zsync"
export OUTNAME="Privacy_Panel-$VERSION-x86_64.AppImage"
/opt/qs/quick-sharun /usr/bin/privacy-panel

install -Dm755 "$HELPER" "$APPDIR/helper/privacy-panel-helper"
install -Dm644 /src/LICENSE "$APPDIR/share/licenses/privacy-panel/LICENSE"
install -Dm644 /src/data/guru.freberg.PrivacyPanel.metainfo.xml "$APPDIR/share/metainfo/guru.freberg.PrivacyPanel.appdata.xml"

# quick-sharun strips with --strip-unneeded, which keeps the .debug_gdb_scripts section
# that Rust programs carry (the glycin image loaders). Remove it so every ELF passes
# tools/check-stripped.sh.
find "$APPDIR" -type f -print0 | while IFS= read -r -d '' f; do
    if [ "$(head -c4 "$f" | od -An -c | tr -d ' ')" = '177ELF' ] \
       && readelf -S "$f" 2>/dev/null | grep -qE '\.symtab|\.debug'; then
        echo "stripping $f"
        strip -s -R .debug_gdb_scripts -R .comment "$f"
    fi
done

/opt/qs/quick-sharun --make-appimage

chown -R "$HOST_UID:$HOST_GID" /build
