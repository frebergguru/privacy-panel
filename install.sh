#!/bin/bash
# Build Privacy Panel and install it system-wide (asks for the administrator password).
#   ./install.sh            build in release mode and install under /usr
#   ./install.sh uninstall  remove the installed files
set -euo pipefail
cd "$(dirname "$0")"

BIN=/usr/bin/privacy-panel
HELPER=/usr/lib/privacy-panel/privacy-panel-helper
POLICY=/usr/share/polkit-1/actions/guru.freberg.PrivacyPanel.policy
DESKTOP=/usr/share/applications/guru.freberg.PrivacyPanel.desktop

if [ "${1:-}" = "uninstall" ]; then
    sudo rm -f "$BIN" "$HELPER" "$POLICY" "$DESKTOP"
    sudo rmdir /usr/lib/privacy-panel 2>/dev/null || true
    echo "Removed. Settings changed with Privacy Panel stay as they are; its history is in ~/.local/state/privacy-panel."
    exit 0
fi

# Clean build: remove old release outputs first so nothing stale is installed.
rm -f target/release/privacy-panel target/release/privacy-panel-helper
cargo build --release -p privacy-panel -p privacy-panel-helper

for f in target/release/privacy-panel target/release/privacy-panel-helper; do
    if readelf -S "$f" | grep -qE '\.symtab|\.debug_'; then
        echo "error: $f is not stripped" >&2
        exit 1
    fi
done

sudo install -Dm755 target/release/privacy-panel "$BIN"
sudo install -Dm755 target/release/privacy-panel-helper "$HELPER"
sudo install -Dm644 data/polkit/guru.freberg.PrivacyPanel.policy "$POLICY"
sudo install -Dm644 data/guru.freberg.PrivacyPanel.desktop "$DESKTOP"
echo "Installed. Start Privacy Panel from the app menu or run: privacy-panel"
