//! Find installed apps (.desktop files) and running processes.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::backends::ini;
use crate::env::Env;
use crate::model::{App, AppSource};

#[derive(Clone, Debug)]
pub struct DesktopEntry {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub comment: Option<String>,
    pub exec: Option<String>,
    pub no_display: bool,
    pub flatpak: Option<String>,
    pub snap: Option<String>,
    pub path: PathBuf,
}

/// Everything found on the machine.
#[derive(Clone, Debug, Default)]
pub struct Inventory {
    pub desktop: BTreeMap<String, DesktopEntry>,
    pub running: HashSet<String>,
    pub flatpaks: Vec<crate::backends::flatpak::FlatpakApp>,
}

impl Inventory {
    pub fn has_desktop(&self, id: &str) -> bool {
        self.desktop.contains_key(id)
    }
    pub fn has_flatpak(&self, id: &str) -> bool {
        self.flatpaks.iter().any(|f| f.id == id)
    }
    pub fn is_running(&self, names: &[String]) -> bool {
        names.iter().any(|n| self.running.contains(n))
    }
}

pub fn desktop_dirs(env: &Env) -> Vec<(PathBuf, Option<&'static str>)> {
    // Lowest priority first; later entries with the same id win.
    vec![
        (env.sys("/usr/share/applications"), None),
        (env.sys("/usr/local/share/applications"), None),
        (env.sys("/var/lib/snapd/desktop/applications"), Some("snap")),
        (env.sys("/var/lib/flatpak/exports/share/applications"), Some("flatpak")),
        (env.data.join("flatpak/exports/share/applications"), Some("flatpak")),
        (env.data.join("applications"), None),
    ]
}

pub fn parse_desktop(id: &str, path: &Path, text: &str) -> Option<DesktopEntry> {
    let g = "Desktop Entry";
    if ini::get(text, g, "Type").as_deref() != Some("Application") {
        return None;
    }
    if ini::get(text, g, "Hidden").as_deref() == Some("true") {
        return None;
    }
    let name = ini::get(text, g, "Name")?;
    Some(DesktopEntry {
        id: id.to_string(),
        name,
        icon: ini::get(text, g, "Icon"),
        comment: ini::get(text, g, "Comment").or_else(|| ini::get(text, g, "GenericName")),
        exec: ini::get(text, g, "Exec"),
        no_display: ini::get(text, g, "NoDisplay").as_deref() == Some("true"),
        flatpak: ini::get(text, g, "X-Flatpak"),
        snap: ini::get(text, g, "X-SnapInstanceName"),
        path: path.to_path_buf(),
    })
}

fn scan_dir(dir: &Path, prefix: &str, out: &mut BTreeMap<String, DesktopEntry>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            scan_dir(&p, &format!("{prefix}{name}-"), out);
        } else if let Some(stem) = name.strip_suffix(".desktop") {
            let id = format!("{prefix}{stem}");
            if let Ok(text) = std::fs::read_to_string(&p) {
                match parse_desktop(&id, &p, &text) {
                    Some(d) => {
                        out.insert(id, d);
                    }
                    None => {
                        // A higher-priority Hidden=true entry removes the app.
                        if ini::get(&text, "Desktop Entry", "Hidden").as_deref() == Some("true") {
                            out.remove(&id);
                        }
                    }
                }
            }
        }
    }
}

pub fn running_processes(env: &Env) -> HashSet<String> {
    if let Some(p) = &env.processes {
        return p.clone();
    }
    let mut out = HashSet::new();
    let Ok(rd) = std::fs::read_dir("/proc") else { return out };
    for e in rd.flatten() {
        let name = e.file_name();
        if !name.to_string_lossy().bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let p = e.path();
        if let Ok(c) = std::fs::read_to_string(p.join("comm")) {
            out.insert(c.trim().to_string());
        }
        if let Ok(cmd) = std::fs::read(p.join("cmdline")) {
            if let Some(arg0) = cmd.split(|b| *b == 0).next() {
                let a = String::from_utf8_lossy(arg0);
                if let Some(base) = a.rsplit('/').next() {
                    if !base.is_empty() {
                        out.insert(base.to_string());
                    }
                }
            }
        }
    }
    out
}

pub fn scan(env: &Env) -> Inventory {
    let mut desktop = BTreeMap::new();
    for (dir, _) in desktop_dirs(env) {
        scan_dir(&dir, "", &mut desktop);
    }
    Inventory {
        desktop,
        running: running_processes(env),
        flatpaks: crate::backends::flatpak::installed_apps(env),
    }
}

/// The program name from an Exec line, e.g. "/usr/bin/vlc --started-from-file %U" -> "vlc".
pub fn exec_name(exec: &str) -> Option<String> {
    let mut it = exec.split_whitespace();
    let mut first = it.next()?;
    if first == "env" {
        first = it.find(|t| !t.contains('='))?;
    }
    if first.ends_with("flatpak") {
        return None;
    }
    first.rsplit('/').next().map(|s| s.trim_matches('"').to_string())
}

/// Turn a desktop entry into an App for the "all installed apps" list.
pub fn app_from_desktop(d: &DesktopEntry) -> App {
    let source = if d.flatpak.is_some() {
        AppSource::Flatpak
    } else if d.snap.is_some() {
        AppSource::Snap
    } else {
        AppSource::Native
    };
    App {
        id: d.id.clone(),
        name: d.name.clone(),
        icon: d.icon.clone(),
        comment: d.comment.clone(),
        source,
        desktop_ids: vec![d.id.clone()],
        processes: d.exec.as_deref().and_then(exec_name).into_iter().collect(),
        sandbox_id: d.flatpak.clone().or_else(|| d.snap.clone()),
        about: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_parsing() {
        let t = "[Desktop Entry]\nType=Application\nName=VLC media player\nName[nb]=VLC\nExec=/usr/bin/vlc --started-from-file %U\nIcon=vlc\n";
        let d = parse_desktop("vlc", Path::new("/x/vlc.desktop"), t).unwrap();
        assert_eq!(d.name, "VLC media player");
        assert_eq!(exec_name(d.exec.as_deref().unwrap()).as_deref(), Some("vlc"));
        assert_eq!(exec_name("env FOO=1 brave %U").as_deref(), Some("brave"));
        assert_eq!(exec_name("/usr/bin/flatpak run org.x"), None);
        assert!(parse_desktop("x", Path::new("/x"), "[Desktop Entry]\nType=Link\nName=x\n").is_none());
    }
}
