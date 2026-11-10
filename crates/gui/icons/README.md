# Built-in icons

Compiled into the binary by `build.rs` (GResource under
`/guru/freberg/PrivacyPanel/icons`), so they show even where the icon theme lacks them,
for example in the AppImage on an older distribution. An installed icon theme that has
the same name still takes precedence.

- `scalable/actions/*.svg`: symbolic icons from the Adwaita icon theme 50.0
  (GNOME Project), licensed CC-BY-SA-3.0 or LGPL-3.0-only.
  https://gitlab.gnome.org/GNOME/adwaita-icon-theme
- `scalable/apps/guru.freberg.PrivacyPanel.svg`: the Privacy Panel icon (same file as
  `data/icons/`), GPL-3.0-or-later like the rest of Privacy Panel.

To add an icon: copy the SVG into `scalable/actions/` and rebuild.
