#!/bin/bash
# Build the Privacy Panel AppImage for x86_64.
#
#   tools/build-appimage.sh          ->  dist/Privacy_Panel-<version>-x86_64.AppImage (+ .zsync)
#
# Privacy Panel needs GTK 4.14+ and libadwaita 1.6+, which older distributions do not
# have, so the AppImage carries the whole GTK stack. It is built in an archlinux
# container with quick-sharun (github.com/pkgforge-dev/Anylinux-AppImages), which also
# bundles glibc and its loader. The AppImage therefore does not depend on the host's
# glibc and starts on old distributions too. Test it on some with
# tools/test-appimage.sh.
#
# Needs Docker. Every run starts clean: build-appimage/ and old AppImages of this
# version in dist/ are removed first. The release stops if any ELF in the AppImage
# still has symbols or debug info (tools/check-stripped.sh).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[ -n "$VERSION" ] || { echo "cannot read the version from Cargo.toml" >&2; exit 1; }
NAME="Privacy_Panel-$VERSION-x86_64.AppImage"
IMAGE=archlinux:latest

command -v docker >/dev/null || { echo "docker is needed" >&2; exit 2; }

# Clean build tree. The cache (pacman packages, rustup, cargo registry) is kept, as it
# holds downloads only, never build output.
rm -rf build-appimage/out
rm -f "dist/$NAME" "dist/$NAME.zsync"
mkdir -p build-appimage/out build-appimage/cache/pacman build-appimage/cache/rustup \
         build-appimage/cache/cargo dist

docker pull -q "$IMAGE" >/dev/null
docker run --rm \
    -e VERSION="$VERSION" -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
    -v "$PWD:/src:ro" \
    -v "$PWD/build-appimage/out:/build" \
    -v "$PWD/build-appimage/cache/pacman:/var/cache/pacman/pkg" \
    -v "$PWD/build-appimage/cache/rustup:/root/.rustup" \
    -v "$PWD/build-appimage/cache/cargo:/root/.cargo/registry" \
    "$IMAGE" bash /src/tools/appimage/inside.sh

OUT=build-appimage/out/dist
set -- "$OUT"/*.AppImage
[ -f "$1" ] || { echo "no AppImage was made" >&2; exit 1; }
[ "$#" -eq 1 ] || { echo "more than one AppImage in $OUT: $*" >&2; exit 1; }
APPIMAGE=$1

# Every ELF that ships must be stripped: the AppImage runtime itself and everything
# inside it (bundled libraries, glibc, the helper).
tools/check-stripped.sh "$APPIMAGE" build-appimage/out/work/AppDir

# Fresh and the right version.
case "$(basename "$APPIMAGE")" in
    *"$VERSION"*) ;;
    *) echo "the AppImage name does not carry version $VERSION: $APPIMAGE" >&2; exit 1 ;;
esac
if [ -n "$(find "$APPIMAGE" -mmin +60)" ]; then
    echo "$APPIMAGE is older than this build" >&2
    exit 1
fi

mv "$APPIMAGE" "dist/$NAME"
[ -f "$APPIMAGE.zsync" ] && mv "$APPIMAGE.zsync" "dist/$NAME.zsync"
chmod +x "dist/$NAME"
ls -l dist/
sha256sum "dist/$NAME"
