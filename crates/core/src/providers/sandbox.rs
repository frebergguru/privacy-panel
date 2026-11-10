//! Per-app permissions: XDG portal grants (camera, microphone, location...), Flatpak
//! sandbox permissions, and Snap interface connections.

use super::{setting, Collector};
use crate::backends::{flatpak, system};
use crate::model::{App, AppSource, Category as C, Section as S, Value, ValueMap};
use crate::target::{read_file, Target};

pub fn collect(c: &mut Collector) {
    flatpaks(c);
    if c.env.live {
        portals(c);
        snaps(c);
    }
}

/// The app id used in the model for a flatpak or portal app id.
fn app_for_sandbox_id(c: &mut Collector, sid: &str) -> String {
    if let Some(a) = c.apps.iter().find(|a| a.sandbox_id.as_deref() == Some(sid) || a.desktop_ids.iter().any(|d| d == sid)) {
        return a.id.clone();
    }
    let entry = c.inv.desktop.get(sid).cloned().or_else(|| {
        c.inv.desktop.values().find(|d| d.flatpak.as_deref() == Some(sid)).cloned()
    });
    let is_flatpak = c.inv.has_flatpak(sid);
    let app = App {
        id: sid.to_string(),
        name: entry.as_ref().map(|e| e.name.clone()).unwrap_or_else(|| sid.to_string()),
        icon: entry.as_ref().and_then(|e| e.icon.clone()),
        comment: entry.as_ref().and_then(|e| e.comment.clone()),
        source: if is_flatpak { AppSource::Flatpak } else { AppSource::Native },
        desktop_ids: entry.iter().map(|e| e.id.clone()).collect(),
        processes: vec![],
        sandbox_id: if is_flatpak { Some(sid.to_string()) } else { None },
        about: None,
    };
    c.app(app);
    sid.to_string()
}

// ---------------------------------------------------------------- Flatpak

struct Perm {
    key: &'static str,
    token: &'static str,
    title: &'static str,
    explanation: &'static str,
}

const KNOWN: &[Perm] = &[
    Perm { key: "shared", token: "network", title: "Network access", explanation: "The app can connect to the internet and your local network." },
    Perm { key: "sockets", token: "x11", title: "X11 display access", explanation: "Apps using X11 can see other X11 windows and read keystrokes typed into them." },
    Perm { key: "sockets", token: "fallback-x11", title: "X11 display access (fallback)", explanation: "X11 access when Wayland is not available." },
    Perm { key: "sockets", token: "pulseaudio", title: "Sound and microphone", explanation: "Access to the sound server, which includes recording from microphones." },
    Perm { key: "sockets", token: "session-bus", title: "Full access to the session bus", explanation: "The app can talk to every other program in your session, which largely defeats the sandbox." },
    Perm { key: "sockets", token: "system-bus", title: "Full access to the system bus", explanation: "The app can talk to every system service." },
    Perm { key: "sockets", token: "ssh-auth", title: "SSH agent", explanation: "The app can use your SSH keys through the SSH agent." },
    Perm { key: "sockets", token: "gpg-agent", title: "GPG agent", explanation: "The app can use your GPG keys through the GPG agent." },
    Perm { key: "sockets", token: "cups", title: "Printing system", explanation: "Direct access to the printing system." },
    Perm { key: "sockets", token: "pcsc", title: "Smart cards", explanation: "Access to smart card readers." },
    Perm { key: "devices", token: "all", title: "All devices", explanation: "Access to every device, including cameras, USB devices and game controllers." },
    Perm { key: "devices", token: "kvm", title: "Virtualisation", explanation: "Access to the virtual machine device." },
    Perm { key: "devices", token: "shm", title: "Shared memory", explanation: "Access to shared memory in /dev/shm." },
    Perm { key: "features", token: "devel", title: "Development tools", explanation: "Allows debugging and tracing system calls." },
    Perm { key: "features", token: "bluetooth", title: "Bluetooth", explanation: "Direct access to Bluetooth." },
    Perm { key: "filesystems", token: "host", title: "All your files and system files", explanation: "Read and write access to almost the whole file system, including your home folder." },
    Perm { key: "filesystems", token: "host-os", title: "System files", explanation: "Access to the operating system's files." },
    Perm { key: "filesystems", token: "host-etc", title: "System configuration", explanation: "Access to /etc." },
    Perm { key: "filesystems", token: "home", title: "Your home folder", explanation: "Read and write access to all files in your home folder." },
];

fn fs_title(token: &str) -> String {
    let t = token.split(':').next().unwrap_or(token);
    let ro = token.ends_with(":ro");
    let name = match t {
        "xdg-download" => "Downloads folder".to_string(),
        "xdg-documents" => "Documents folder".to_string(),
        "xdg-music" => "Music folder".to_string(),
        "xdg-pictures" => "Pictures folder".to_string(),
        "xdg-videos" => "Videos folder".to_string(),
        "xdg-desktop" => "Desktop folder".to_string(),
        "xdg-config" => "Settings of other apps".to_string(),
        "xdg-data" => "Data of other apps".to_string(),
        "xdg-cache" => "Caches of other apps".to_string(),
        "xdg-run" => "Runtime files".to_string(),
        other => format!("Folder {other}"),
    };
    if ro {
        format!("{name} (read only)")
    } else {
        name
    }
}

fn flatpaks(c: &mut Collector) {
    let apps = c.inv.flatpaks.clone();
    for fa in apps {
        let md = read_file(&fa.metadata).unwrap_or_default();
        let app_id = app_for_sandbox_id(c, &fa.id);
        if let Some(a) = c.apps.iter_mut().find(|a| a.id == app_id) {
            a.source = AppSource::Flatpak;
            a.sandbox_id = Some(fa.id.clone());
        }
        let note = "Takes effect the next time the app starts. Turning a permission off can stop parts of the app from working; turn it back on if that happens.";
        for key in ["shared", "sockets", "devices", "features", "filesystems"] {
            let mut tokens: Vec<String> = flatpak::static_tokens(&md, key);
            for t in flatpak::effective_tokens(c.env, &fa, key) {
                if !tokens.iter().any(|x| x.split(':').next() == t.split(':').next()) {
                    tokens.push(t);
                }
            }
            if let Some(v) = crate::backends::ini::get(
                &read_file(&flatpak::user_override_path(c.env, &fa.id)).unwrap_or_default(),
                "Context",
                key,
            ) {
                for t in flatpak::split_list(&v) {
                    let t = t.trim_start_matches('!').to_string();
                    if !tokens.iter().any(|x| x.split(':').next() == t.split(':').next()) {
                        tokens.push(t);
                    }
                }
            }
            for tok in tokens {
                let base = tok.split(':').next().unwrap_or(&tok).to_string();
                if matches!(base.as_str(), "ipc" | "wayland" | "dri" | "inherit-wayland-socket") {
                    continue;
                }
                let known = KNOWN.iter().find(|p| p.key == key && p.token == base);
                let (title, expl) = match known {
                    Some(p) => (p.title.to_string(), p.explanation.to_string()),
                    None if key == "filesystems" => (fs_title(&tok), format!("Access to {tok}.")),
                    None => (format!("{key}: {base}"), format!("Flatpak permission {key}={base}.")),
                };
                let cat = match key {
                    "shared" => C::Network,
                    "filesystems" => C::Sandbox,
                    "devices" | "features" => C::Sharing,
                    _ => C::Sandbox,
                };
                c.add(setting(format!("flatpak.{}.{key}.{base}", fa.id), &app_id, S::Sandbox, cat, title,
                    format!("{expl} {note}"))
                    .boolean(
                        Target::Flatpak {
                            app: fa.id.clone(),
                            metadata: fa.metadata.clone(),
                            key: key.into(),
                            token: base.clone(),
                            grant: Some(tok.clone()),
                        },
                        ValueMap::bool("on", "off"),
                        None,
                        None,
                    )
                    .group("Sandbox permissions"));
            }
        }
    }
}

// ---------------------------------------------------------------- Portals

const TABLES: &[(&str, &str, C)] = &[
    ("devices", "Devices", C::Sharing),
    ("location", "Location", C::Location),
    ("notifications", "Notifications", C::Sharing),
    ("background", "Background", C::Sharing),
    ("screenshot", "Screenshots", C::Sharing),
    ("remote-desktop", "Remote desktop", C::Sharing),
    ("screencast", "Screen sharing", C::Sharing),
    ("kde-authorized", "KDE", C::Sharing),
    ("gamemode", "Game mode", C::Sharing),
    ("realtime", "Real-time scheduling", C::Sharing),
];

fn portal_title(table: &str, id: &str) -> (String, String) {
    match (table, id) {
        ("devices", "camera") => ("Camera".into(), "The app may use the camera without asking again.".into()),
        ("devices", "microphone") => ("Microphone".into(), "The app may record from the microphone without asking again.".into()),
        ("devices", "speakers") => ("Speakers".into(), "The app may play sound.".into()),
        ("location", _) => ("Location".into(), "The app may get your location without asking again.".into()),
        ("notifications", _) => ("Notifications".into(), "The app may show notifications.".into()),
        ("background", _) => ("Run in the background".into(), "The app may keep running when its windows are closed, and start when you log in.".into()),
        ("screenshot", _) => ("Take screenshots".into(), "The app may take screenshots of your whole screen without asking again.".into()),
        ("remote-desktop", _) => ("Remote desktop".into(), "The app may see your screen and control mouse and keyboard.".into()),
        ("screencast", _) => ("Screen sharing".into(), "The app may record or share your screen.".into()),
        ("kde-authorized", "remote-desktop") => ("Remote control".into(), "The app may control this computer without asking.".into()),
        ("kde-authorized", other) => (format!("KDE: {other}"), format!("Permission \"{other}\" granted through KDE.")),
        (t, i) => (format!("{t}: {i}"), format!("Portal permission {t}/{i}.")),
    }
}

fn portals(c: &mut Collector) {
    for (table, _tname, cat) in TABLES {
        let Ok(ids) = system::portal_list(table) else { continue };
        for id in ids {
            let Ok(perms) = system::portal_lookup(table, &id) else { continue };
            for (sid, val) in perms {
                if sid.is_empty() {
                    continue;
                }
                let app_id = app_for_sandbox_id(c, &sid);
                let (title, expl) = portal_title(table, &id);
                let (on, off) = if *table == "location" {
                    (r#"["EXACT","0"]"#, r#"["NONE","0"]"#)
                } else {
                    (r#"["yes"]"#, r#"["no"]"#)
                };
                let mut s = setting(format!("portal.{table}.{id}.{sid}"), &app_id, S::Permissions, *cat, title,
                    format!("{expl} Granted through the desktop's permission prompts. Turning it off makes the app ask again or be refused."))
                    .boolean(Target::Portal { table: table.to_string(), id: id.clone(), app: sid.clone() }, ValueMap::bool(on, off), Some(false), None)
                    .group("Granted permissions");
                // Any stored grant that is not a refusal counts as on (e.g. location with a timestamp).
                let raw = serde_json::to_string(&val).unwrap_or_default();
                let refused = val.first().is_some_and(|f| f == "no" || f == "NONE" || f == "ask");
                if !refused {
                    s = s.alias(&raw, Value::Bool(true));
                } else {
                    s = s.alias(&raw, Value::Bool(false));
                }
                c.add(s);
            }
        }
    }
}

// ---------------------------------------------------------------- Snap

fn snaps(c: &mut Collector) {
    if !c.env.has_binary("snap") {
        return;
    }
    let Ok(rows) = system::snap_connections() else { return };
    let sensitive = [
        ("camera", "Camera"),
        ("audio-record", "Microphone"),
        ("home", "Your home folder"),
        ("removable-media", "USB drives and other removable media"),
        ("location-observe", "Location"),
        ("network", "Network access"),
        ("network-observe", "See network activity"),
        ("personal-files", "Personal files"),
        ("password-manager-service", "Password manager"),
        ("screen-inhibit-control", "Keep the screen on"),
        ("x11", "X11 display access"),
        ("raw-usb", "Raw USB devices"),
        ("joystick", "Game controllers"),
        ("bluez", "Bluetooth"),
    ];
    for r in rows {
        let Some((_, title)) = sensitive.iter().find(|(i, _)| *i == r.interface) else { continue };
        let snap = r.plug.split(':').next().unwrap_or("").to_string();
        if snap.is_empty() {
            continue;
        }
        let app_id = app_for_sandbox_id(c, &snap);
        if let Some(a) = c.apps.iter_mut().find(|a| a.id == app_id) {
            a.source = AppSource::Snap;
        }
        c.add(setting(format!("snap.{}", r.plug), &app_id, S::Sandbox, C::Sandbox, *title,
            format!("Snap interface \"{}\". snapd asks for your password to change it. Takes effect the next time the app starts.", r.interface))
            .boolean(Target::Snap { plug: r.plug.clone(), slot: r.slot.clone() }, ValueMap::bool("connected", "disconnected"), None, None)
            .group("Snap permissions"));
    }
}
