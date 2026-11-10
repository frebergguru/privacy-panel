//! Icon lookup that never shows the "missing image" icon.
//!
//! Icon themes differ: Breeze, Adwaita and Papirus each lack names the others have, and
//! desktop files may give a file path or a name with an extension. Every icon the app
//! shows goes through here and falls back to a name the current theme has.

use gtk::{gdk, gio, prelude::*};
use std::path::Path;

/// UI icons: the first name is the preferred one, the rest are fallbacks for themes that
/// lack it (checked against Breeze, Adwaita and Papirus).
pub const BROWSERS: &[&str] = &["web-browser-symbolic", "internet-web-browser-symbolic", "applications-internet-symbolic", "globe-symbolic"];
pub const SANDBOX: &[&str] = &["package-x-generic-symbolic", "package-symbolic", "application-x-addon-symbolic", "folder-symbolic"];
pub const APPLICATIONS: &[&str] = &["applications-other-symbolic", "view-app-grid-symbolic", "view-grid-symbolic"];
pub const LIST: &[&str] = &["view-list-symbolic", "view-list-details-symbolic", "view-grid-symbolic"];
pub const OK: &[&str] = &["emblem-ok-symbolic", "object-select-symbolic", "checkbox-checked-symbolic"];
pub const WARNING: &[&str] = &["dialog-warning-symbolic", "data-warning-symbolic", "emblem-important-symbolic"];
pub const QUESTION: &[&str] = &["dialog-question-symbolic", "help-contents-symbolic", "help-about-symbolic"];
pub const INFO: &[&str] = &["dialog-information-symbolic", "data-information-symbolic", "help-about-symbolic"];
/// The app's own icon (About dialog).
pub const APP: &[&str] = &["guru.freberg.PrivacyPanel", "security-high", "security-high-symbolic", "preferences-system-privacy", "changes-prevent-symbolic"];

const APP_FALLBACK: &[&str] = &["application-x-executable", "application-x-executable-symbolic", "applications-other-symbolic", "image-missing"];

fn theme() -> Option<gtk::IconTheme> {
    gdk::Display::default().map(|d| gtk::IconTheme::for_display(&d))
}

/// The first of `names` the current theme has, else the last one.
pub fn resolve(names: &[&str]) -> String {
    let th = theme();
    names
        .iter()
        .find(|n| th.as_ref().is_none_or(|t| t.has_icon(n)))
        .or(names.last())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Icon for an app: a theme name, a name with an image extension, or a file path; falls
/// back to a generic program icon when the theme has none of these.
pub fn app_gicon(icon: Option<&str>) -> gio::Icon {
    if let Some(i) = icon.map(str::trim).filter(|i| !i.is_empty()) {
        if i.starts_with('/') {
            if Path::new(i).is_file() {
                return gio::FileIcon::new(&gio::File::for_path(i)).upcast();
            }
        } else {
            let th = theme();
            let has = |n: &str| th.as_ref().is_none_or(|t| t.has_icon(n));
            if has(i) {
                return gio::ThemedIcon::new(i).upcast();
            }
            if let Some(stem) = [".png", ".svg", ".svgz", ".xpm"].iter().find_map(|e| i.strip_suffix(e)) {
                if has(stem) {
                    return gio::ThemedIcon::new(stem).upcast();
                }
            }
        }
    }
    gio::ThemedIcon::new(&resolve(APP_FALLBACK)).upcast()
}

pub fn app_image(icon: Option<&str>, size: i32) -> gtk::Image {
    let img = gtk::Image::from_gicon(&app_gicon(icon));
    img.set_pixel_size(size);
    img
}

/// A paintable for an app icon, for widgets that take one (AdwStatusPage).
pub fn app_paintable(icon: Option<&str>, size: i32, widget: &impl IsA<gtk::Widget>) -> Option<gtk::IconPaintable> {
    let th = theme()?;
    Some(th.lookup_by_gicon(&app_gicon(icon), size, widget.scale_factor(), gtk::TextDirection::None, gtk::IconLookupFlags::empty()))
}

pub fn image(names: &[&str]) -> gtk::Image {
    gtk::Image::from_icon_name(&resolve(names))
}
