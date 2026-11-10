#!/bin/bash
# Start the AppImage on old distributions, in containers, without a visible window.
#
#   tools/test-appimage.sh [dist/Privacy_Panel-*.AppImage] [image ...]
#
# Each container gets a font package, as every desktop has one.
# For each image: the app scans the (empty) container home with --dump-json, then
# renders every page with --selftest-screenshot under Xvfb. The screenshots land in
# build-appimage/test/<image>/ so they can be looked at. Containers have no FUSE, so
# the AppImage runs with --appimage-extract-and-run.
set -euo pipefail
cd "$(dirname "$0")/.."

APPIMAGE=${1:-$(ls -t dist/*.AppImage | head -1)}
shift || true
IMAGES=("$@")
[ "${#IMAGES[@]}" -gt 0 ] || IMAGES=(ubuntu:18.04 ubuntu:20.04 fedora:36)

fail=0
for img in "${IMAGES[@]}"; do
    out="build-appimage/test/${img//[:\/]/_}"
    rm -rf "$out"
    mkdir -p "$out"
    echo "== $img"
    if docker run --rm \
        -v "$PWD/$APPIMAGE:/app.AppImage:ro" -v "$PWD/$out:/out" \
        -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
        "$img" bash -c '
            set -e
            if command -v apt-get >/dev/null; then
                export DEBIAN_FRONTEND=noninteractive
                apt-get update -qq >/dev/null && apt-get install -y -qq xvfb xauth dbus fonts-dejavu-core >/dev/null
            elif command -v dnf >/dev/null; then
                dnf install -y -q xorg-x11-server-Xvfb xorg-x11-xauth util-linux dbus-daemon dejavu-sans-fonts >/dev/null
            fi
            ldd --version | head -1
            export APPIMAGE_EXTRACT_AND_RUN=1 NO_CLEANUP=1
            cp /app.AppImage /tmp/app && chmod +x /tmp/app
            /tmp/app --dump-json > /out/dump.json
            echo "dump-json: $(grep -c "\"id\"" /out/dump.json) ids"
            # A desktop always has a machine id and a session bus; a bare container
            # does not, and GtkApplication needs the bus.
            [ -s /etc/machine-id ] || tr -d - < /proc/sys/kernel/random/uuid > /etc/machine-id
            dbus-run-session -- xvfb-run -a -s "-screen 0 1280x900x24" /tmp/app --selftest-screenshot /out/shots 2>/out/stderr.txt
            n=$(ls /out/shots 2>/dev/null | wc -l)
            echo "screenshots: $n"
            chown -R "$HOST_UID:$HOST_GID" /out
            [ "$n" -ge 11 ] || { echo "the window did not render every page" >&2; exit 1; }
            chown -R "$HOST_UID:$HOST_GID" /out
        '; then
        echo "   ok"
    else
        echo "   FAILED (see $out)"
        fail=1
    fi
done
exit $fail
