//! Desktop-wide settings: KDE Plasma, GNOME, GTK, KDE Connect, and clean-up actions.

use super::{setting, system_app, Collector};
use crate::backends::{ini, system};
use crate::model::{ActionKind, Category as C, Section as S, ValueMap};
use crate::target::{read_file, Target};

fn kcfg(c: &Collector, file: &str, group: &str, key: &str) -> Target {
    Target::Ini { path: c.env.config.join(file), group: group.into(), key: key.into(), kde_notify: true }
}

fn tf() -> ValueMap {
    ValueMap::bool("true", "false")
}

pub fn collect(c: &mut Collector) {
    if c.env.is_kde() {
        kde(c);
        kde_connect(c);
    }
    if c.env.is_gnome() && c.env.live {
        gnome(c);
    }
    gtk(c);
    cleanup(c);
}

fn kde(c: &mut Collector) {
    let app = "kde-plasma";
    c.app(system_app(app, "KDE Plasma desktop", "plasma", "Settings of the Plasma desktop and KDE apps"));
    let list = vec![
        setting("kde.baloo", app, S::System, C::History, "File search indexing (Baloo)",
            "Baloo reads the files in your home folder and builds a search index of their names and contents. The index stays on this computer, but any program running as you can search it. Turning it off makes file search slower and less complete.")
            .boolean(kcfg(c, "baloofilerc", "Basic Settings", "Indexing-Enabled"), tf(), Some(true), None),
        setting("kde.baloo-basic", app, S::System, C::History, "Index file names only, not contents",
            "When file indexing is on, only file names are indexed and the contents of your documents are left alone.")
            .boolean(kcfg(c, "baloofilerc", "General", "only basic indexing"), tf(), Some(false), Some(true)),
        setting("kde.userfeedback", app, S::System, C::Telemetry, "Allow KDE user feedback",
            "The master switch for KDE's feedback system. When off, no KDE program sends usage statistics or shows feedback surveys. KDE never sends anything unless you choose a feedback level.")
            .boolean(Target::Ini { path: c.env.config.join("KDE/UserFeedback.conf"), group: "UserFeedback".into(), key: "Enabled".into(), kde_notify: false }, tf(), Some(true), Some(false)),
        setting("kde.feedback-level", app, S::System, C::Telemetry, "Plasma feedback level",
            "How much information Plasma sends to KDE about your system and how you use it.")
            .choice(kcfg(c, "PlasmaUserFeedback", "Global", "FeedbackLevel"), &[
                ("0", "Off", Some("0")),
                ("16", "Basic system information", Some("16")),
                ("32", "Basic system information and usage", Some("32")),
                ("48", "Detailed system information", Some("48")),
                ("64", "Detailed system information and usage", Some("64")),
            ], Some("0"), Some("0")),
        setting("kde.activity-remember", app, S::System, C::History, "Remember which files and apps you use",
            "Plasma keeps a history of recently opened files and apps for menus, search and the \"Recent\" lists. It stays on this computer.")
            .choice(kcfg(c, "kactivitymanagerd-pluginsrc", "Plugin-org.kde.ActivityManager.Resources.Scoring", "what-to-remember"), &[
                ("0", "For all apps", Some("0")),
                ("1", "Only for some apps", Some("1")),
                ("2", "Do not remember", Some("2")),
            ], Some("0"), None),
        setting("kde.activity-keep", app, S::System, C::History, "How long to keep the usage history",
            "Older entries are forgotten automatically.")
            .choice(kcfg(c, "kactivitymanagerd-pluginsrc", "Plugin-org.kde.ActivityManager.Resources.Scoring", "keep-history-for"), &[
                ("0", "Forever", Some("0")),
                ("1", "1 month", Some("1")),
                ("3", "3 months", Some("3")),
                ("6", "6 months", Some("6")),
                ("12", "1 year", Some("12")),
            ], Some("0"), None),
        setting("kde.recent-docs", app, S::System, C::History, "Remember recently opened documents in KDE apps",
            "KDE apps keep a list of documents you opened recently, shown in their File menus.")
            .boolean(kcfg(c, "kdeglobals", "RecentDocuments", "UseRecent"), tf(), Some(true), None),
        setting("kde.clipboard-keep", app, S::System, C::History, "Save clipboard history to disk",
            "Keeps the clipboard history after you log out. Passwords and other secrets you copy are then stored on disk.")
            .boolean(kcfg(c, "klipperrc", "General", "KeepClipboardContents"), tf(), Some(true), Some(false)),
        setting("kde.autolock", app, S::System, C::Security, "Lock the screen automatically",
            "Locks the screen after a period without activity, so nobody can use your session while you are away.")
            .boolean(kcfg(c, "kscreenlockerrc", "Daemon", "Autolock"), tf(), Some(true), Some(true)),
        setting("kde.lock-resume", app, S::System, C::Security, "Lock the screen after waking from sleep",
            "Asks for your password when the computer wakes up.")
            .boolean(kcfg(c, "kscreenlockerrc", "Daemon", "LockOnResume"), tf(), Some(true), Some(true)),
        setting("kde.wallet-screensaver", app, S::System, C::Security, "Close the password wallet when the screen locks",
            "KWallet stores passwords for apps. Closing it when the screen locks means it has to be opened again with your wallet password.")
            .boolean(kcfg(c, "kwalletrc", "Wallet", "Close on Screensaver"), tf(), Some(false), None),
    ];
    for s in list {
        c.add(s);
    }
}

fn kde_connect(c: &mut Collector) {
    let dir = c.env.config.join("kdeconnect");
    let trusted = read_file(&dir.join("trusted_devices")).unwrap_or_default();
    let devices = ini::groups(&trusted);
    let installed = c.env.has_binary("kdeconnectd")
        || c.inv.has_desktop("org.kde.kdeconnect.app")
        || c.env.sys("/usr/lib/kdeconnectd").exists();
    if !installed {
        return;
    }
    let app = "kdeconnect";
    let a = c.known_app(app, "KDE Connect", "kdeconnect",
        &["org.kde.kdeconnect.app", "org.kde.kdeconnect.daemon", "org.kde.kdeconnect.nonplasma", "org.kde.kdeconnect.sms", "org.kde.kdeconnect.handler"],
        &["kdeconnectd"]);
    c.app(a);
    if devices.is_empty() {
        c.add(setting("kdeconnect.devices", app, S::Applications, C::Sharing, "Paired devices",
            "Phones and computers that can connect to this computer through KDE Connect.").info("No paired devices"));
        return;
    }
    let names: Vec<String> = devices
        .iter()
        .map(|d| ini::get(&trusted, d, "name").unwrap_or_else(|| d.clone()))
        .collect();
    c.add(setting("kdeconnect.devices", app, S::Applications, C::Sharing, "Paired devices",
        "Phones and computers that can connect to this computer through KDE Connect. Unpair devices you no longer use in KDE Connect's own settings.")
        .info(names.join(", ")));
    let plugins: [(&str, &str, &str, bool, Option<bool>); 5] = [
        ("kdeconnect_clipboard", "Share the clipboard", "Everything you copy is sent to the device, and the other way round. That includes passwords.", true, Some(false)),
        ("kdeconnect_sendnotifications", "Send this computer's notifications to the device", "Notifications can contain message text and other private details.", false, None),
        ("kdeconnect_mousepad", "Let the device control mouse and keyboard", "The device can type and click on this computer.", true, None),
        ("kdeconnect_runcommand", "Let the device run commands", "Commands you have set up can be started from the device.", true, None),
        ("kdeconnect_sftp", "Browse the device's files", "Lets this computer open the files on the paired phone.", true, None),
    ];
    for (dev, name) in devices.iter().zip(names) {
        for (p, title, expl, def, rec) in plugins {
            c.add(setting(format!("kdeconnect.{dev}.{p}"), app, S::Applications, C::Sharing, title,
                format!("{expl} Applies after KDE Connect restarts."))
                .boolean(Target::Ini { path: dir.join(dev).join("config"), group: "Plugins".into(), key: format!("{p}Enabled"), kde_notify: false }, tf(), Some(def), rec)
                .group(format!("Device: {name}")));
        }
    }
}

fn gnome(c: &mut Collector) {
    let app = "gnome";
    c.app(system_app(app, "GNOME desktop", "org.gnome.Settings", "Settings of the GNOME desktop"));
    let env = c.env;
    let gs = |schema: &str, key: &str| Target::GSettings { schema: schema.into(), key: key.into() };
    let has = |schema: &str, key: &str| system::gsettings_has_key(env, schema, key);
    let p = "org.gnome.desktop.privacy";
    let bools: [(&str, &str, &str, &str, C, Option<bool>); 10] = [
        (p, "remember-recent-files", "Remember recently used files", "GNOME apps keep a list of files you opened recently.", C::History, None),
        (p, "remember-app-usage", "Remember which apps you use", "Used to sort apps by how often you use them.", C::History, None),
        (p, "report-technical-problems", "Automatic problem reporting", "Sends technical reports about crashes and errors to your distribution.", C::Telemetry, Some(false)),
        (p, "send-software-usage-stats", "Send software usage statistics", "Sends anonymous statistics about which software you use.", C::Telemetry, Some(false)),
        (p, "remove-old-trash-files", "Empty old files from the trash automatically", "Deleted files are removed for good after a set number of days.", C::History, None),
        (p, "remove-old-temp-files", "Remove old temporary files automatically", "Temporary files are removed after a set number of days.", C::History, None),
        (p, "disable-camera", "Block the camera for all apps", "Stops every app from using the camera until you turn this off.", C::Sharing, None),
        (p, "disable-microphone", "Block the microphone for all apps", "Stops every app from using the microphone until you turn this off.", C::Sharing, None),
        ("org.gnome.system.location", "enabled", "Location services", "Lets apps find your location, usually by sending nearby Wi-Fi networks to an online service.", C::Location, Some(false)),
        ("org.gnome.desktop.screensaver", "lock-enabled", "Lock the screen automatically", "Locks the screen when the screen saver starts.", C::Security, Some(true)),
    ];
    for (schema, key, title, expl, cat, rec) in bools {
        if has(schema, key) {
            c.add(setting(format!("gnome.{schema}.{key}"), app, S::System, cat, title, expl)
                .boolean(gs(schema, key), tf(), None, rec));
        }
    }
    if has("org.gnome.desktop.notifications", "show-in-lock-screen") {
        c.add(setting("gnome.notif-lock", app, S::System, C::Security, "Show notifications on the lock screen",
            "Anyone near the computer can read notifications while it is locked.")
            .boolean(gs("org.gnome.desktop.notifications", "show-in-lock-screen"), tf(), None, Some(false)));
    }
    if has(p, "recent-files-max-age") {
        c.add(setting("gnome.recent-age", app, S::System, C::History, "Keep recent files for",
            "How long files stay in the recent files list.")
            .choice(gs(p, "recent-files-max-age"), &[
                ("-1", "Forever", Some("-1")),
                ("1", "1 day", Some("1")),
                ("7", "7 days", Some("7")),
                ("30", "30 days", Some("30")),
            ], None, None));
    }
    if has("org.gnome.desktop.search-providers", "disable-external") {
        c.add(setting("gnome.search-external", app, S::System, C::Tracking, "Search in apps from the Activities overview",
            "Lets apps such as Files, Software and web browsers show results when you search from the overview.")
            .boolean(gs("org.gnome.desktop.search-providers", "disable-external"), ValueMap::bool("false", "true"), None, None));
    }
}

fn gtk(c: &mut Collector) {
    let g3 = c.env.config.join("gtk-3.0/settings.ini");
    let g4 = c.env.config.join("gtk-4.0/settings.ini");
    if !g3.exists() && !g4.exists() {
        return;
    }
    let app = "gtk";
    c.app(system_app(app, "GTK apps", "applications-other", "Settings shared by apps built with GTK (GIMP, Inkscape, Firefox file dialogs and more)"));
    let t = |p: &std::path::Path| Target::Ini { path: p.to_path_buf(), group: "Settings".into(), key: "gtk-recent-files-enabled".into(), kde_notify: false };
    c.add(setting("gtk.recent", app, S::System, C::History, "Remember recently used files in GTK apps",
        "GTK apps add every file you open to a shared list in ~/.local/share/recently-used.xbel. Applies when apps are restarted.")
        .boolean(Target::Multi(vec![t(&g3), t(&g4)]), tf(), Some(true), None));
}

fn cleanup(c: &mut Collector) {
    let app = "cleanup";
    c.app(system_app(app, "Clean up", "edit-clear-all", "One-off clean-up of traces left on this computer"));
    c.add(setting("cleanup.recent", app, S::System, C::History, "Clear the recent files list",
        "Empties the list of recently opened files that GTK apps and KDE share. This cannot be undone.")
        .action(ActionKind::ClearRecentFiles, "Clear"));
    c.add(setting("cleanup.thumbs", app, S::System, C::History, "Delete thumbnail cache",
        "Small previews of images and videos you have viewed are kept in ~/.cache/thumbnails, even after the files are deleted. They are made again when needed. This cannot be undone.")
        .action(ActionKind::ClearThumbnails, "Delete"));
}
