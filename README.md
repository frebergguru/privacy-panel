# Privacy Panel

A GTK4/libadwaita app (Rust) that finds the privacy settings of the apps on a Linux
computer, shows them in one place, and lets you change them one by one, per app, or all
at once. Every change is recorded and can be undone.

## What it covers

- **Desktop:** KDE Plasma (file indexing, KDE feedback/telemetry, activity history,
  recent documents, clipboard history, screen locking, KWallet), KDE Connect (per paired
  device), GNOME (privacy, location, problem reporting, lock screen), GTK recent files,
  plus clean-up actions (recent files list, thumbnail cache).
- **Browsers:** Firefox, LibreWolf, Floorp, Waterfox, Zen and Thunderbird per profile
  (telemetry, studies, sponsored content, tracking protection, HTTPS-only, WebRTC, DNS
  over HTTPS and more). Brave, Chrome, Chromium, Vivaldi, Edge and Opera per profile
  (metrics, background mode, Brave P3A and usage ping, suggestions, preloading, WebRTC,
  cookies). Optional policies for all users (Firefox, Brave, Chrome, Chromium, Edge).
- **Apps** (from `data/apps/*.toml`): VLC, LibreOffice, GIMP, Audacity, Transmission,
  Heroic, FileZilla, VS Code, VSCodium.
- **Permissions:** camera, microphone, location, screenshots, background and so on that
  were granted through the desktop portal; Flatpak sandbox permissions (as user
  overrides, like `flatpak override --user`); Snap interface connections.
- **System (administrator password):** Avahi, cups-browsed, Bluetooth, SSH, GeoClue,
  ModemManager, pkgstats, Apport, Whoopsie, ABRT; NetworkManager MAC address
  randomisation, connectivity check, sending the host name, IPv6 privacy addresses;
  journald storage and retention; core dumps; Debian/Ubuntu popularity contest and
  motd news.
- **Every other installed app** is listed too. When Privacy Panel knows no settings for
  an app, it says so; it never guesses.

Settings that the app overwrites while running (browsers, VLC, LibreOffice,
Transmission...) are locked with "Close ... first" until the app is closed.

## Build and run

Needs Rust 1.87 or newer, GTK 4.14+ and libadwaita 1.6+ (development files).

```sh
cargo build --release
./target/release/privacy-panel
```

Install system-wide (binary, root helper, polkit policy, menu entry):

```sh
./install.sh            # or: ./install.sh uninstall
```

## Adding an app

Copy one of `data/apps/*.toml` to `~/.local/share/privacy-panel/apps/myapp.toml` and
change it. Formats: `ini`, `json` (comments are kept), `xcu`, `gimprc`, `xmlsetting`.
Built-in files are compiled in; rebuild after editing them.

## How changes are made

- User settings are edited in place: only the one line or value concerned changes, and
  comments, order and formatting elsewhere are kept. Symbolic links (dotfile managers)
  are written through, not replaced. Firefox prefs go into a marked block in
  `user.js` plus `prefs.js`.
- Before a file is changed, a copy goes to `~/.local/state/privacy-panel/backups/`.
- History with before/after values: `~/.local/state/privacy-panel/history.jsonl`.
  Menu, History and undo: undo one change, or restore everything.
- System settings go through `privacy-panel-helper`, run with `pkexec` (one password
  prompt per batch). The helper only accepts the exact settings and values listed in
  `crates/core/src/admin.rs`; it never takes a file path or command from the request.

## Tests

```sh
cargo test                     # backends, helper whitelist, fake-home end to end
./target/debug/privacy-panel --dump-json      # what it finds on this computer (read only)
PRIVACY_PANEL_ADMIN_DRY_RUN=1 ./target/debug/privacy-panel   # system changes only logged
```

Headless screenshots: `GDK_BACKEND=broadway` with `gtk4-broadwayd`, then
`privacy-panel --selftest-screenshot DIR`.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
