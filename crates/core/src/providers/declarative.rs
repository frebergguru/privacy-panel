//! Apps described in TOML files: the built-in ones in data/apps/ (compiled in), plus any
//! the user adds in ~/.local/share/privacy-panel/apps/*.toml.

use serde::Deserialize;

use super::{setting, Collector};
use crate::model::{Category, Section, ValueMap};
use crate::target::Target;

const BUILTIN: &[(&str, &str)] = &[
    ("vlc", include_str!("../../../../data/apps/vlc.toml")),
    ("libreoffice", include_str!("../../../../data/apps/libreoffice.toml")),
    ("gimp", include_str!("../../../../data/apps/gimp.toml")),
    ("audacity", include_str!("../../../../data/apps/audacity.toml")),
    ("transmission", include_str!("../../../../data/apps/transmission.toml")),
    ("heroic", include_str!("../../../../data/apps/heroic.toml")),
    ("filezilla", include_str!("../../../../data/apps/filezilla.toml")),
    ("vscode", include_str!("../../../../data/apps/vscode.toml")),
    ("vscodium", include_str!("../../../../data/apps/vscodium.toml")),
];

#[derive(Debug, Deserialize)]
pub struct AppFile {
    pub app: AppDef,
    #[serde(default)]
    pub setting: Vec<SettingDef>,
}

#[derive(Debug, Deserialize)]
pub struct AppDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub desktop: Vec<String>,
    #[serde(default)]
    pub binaries: Vec<String>,
    /// Flatpak id; config paths starting with $XDG_CONFIG_HOME are also looked for in
    /// ~/.var/app/<id>/config.
    #[serde(default)]
    pub flatpak: Option<String>,
    #[serde(default)]
    pub processes: Vec<String>,
    #[serde(default = "default_section")]
    pub section: Section,
    /// What the app is, in plain words.
    #[serde(default)]
    pub about: Option<String>,
}

fn default_section() -> Section {
    Section::Applications
}

#[derive(Debug, Deserialize)]
pub struct OptDef {
    pub id: String,
    pub label: String,
    /// Raw value; leave out for "key absent".
    #[serde(default)]
    pub raw: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SettingDef {
    pub id: String,
    pub title: String,
    pub explanation: String,
    pub category: Category,
    /// ini | json | xcu | gimprc | xmlsetting
    pub format: String,
    /// Candidate files; every one that exists gets its own row. If none exists, the first
    /// is used (and created on first change, except for xcu and xmlsetting).
    pub files: Vec<String>,
    #[serde(default)]
    pub group: String,
    /// ini/gimprc/xmlsetting key, or xcu property name.
    #[serde(default)]
    pub key: String,
    /// json: path of keys.
    #[serde(default)]
    pub pointer: Vec<String>,
    /// xcu: item path.
    #[serde(default)]
    pub item: String,
    /// "bool" or "choice".
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub on: Option<String>,
    #[serde(default)]
    pub off: Option<String>,
    #[serde(default)]
    pub options: Vec<OptDef>,
    /// bool: true/false; choice: an option id.
    #[serde(default)]
    pub default: Option<toml::Value>,
    #[serde(default)]
    pub recommended: Option<toml::Value>,
    #[serde(default)]
    pub requires_closed: bool,
    /// Extra note added to the explanation.
    #[serde(default)]
    pub note: Option<String>,
    /// What the thing is, in plain words ("What is this?").
    #[serde(default)]
    pub about: Option<String>,
}

pub fn parse(text: &str) -> Result<AppFile, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

fn user_files(c: &Collector) -> Vec<(String, String)> {
    let dir = c.env.data.join("privacy-panel/apps");
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "toml") {
                if let Ok(t) = std::fs::read_to_string(&p) {
                    out.push((p.display().to_string(), t));
                }
            }
        }
    }
    out
}

pub fn collect(c: &mut Collector) {
    let mut files: Vec<(String, String)> = BUILTIN.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect();
    files.extend(user_files(c));
    for (name, text) in files {
        match parse(&text) {
            Ok(f) => add_app(c, &f),
            Err(e) => eprintln!("privacy-panel: skipping {name}: {e}"),
        }
    }
}

fn add_app(c: &mut Collector, f: &AppFile) {
    let a = &f.app;
    let by_desktop = a.desktop.iter().any(|d| c.inv.has_desktop(d));
    let by_bin = a.binaries.iter().any(|b| c.env.has_binary(b));
    let by_flatpak = a.flatpak.as_deref().is_some_and(|id| c.inv.has_flatpak(id));
    if !(by_desktop || by_bin || by_flatpak) {
        return;
    }
    let desktop: Vec<&str> = a.desktop.iter().map(String::as_str).collect();
    let procs: Vec<&str> = a.processes.iter().map(String::as_str).collect();
    let mut app = c.known_app(&a.id, &a.name, a.icon.as_deref().unwrap_or("application-x-executable"), &desktop, &procs);
    if a.about.is_some() {
        app.about = a.about.clone();
    }
    if by_flatpak && !by_desktop && !by_bin {
        app.source = crate::model::AppSource::Flatpak;
    }
    if let Some(fp) = &a.flatpak {
        if by_flatpak {
            app.sandbox_id = Some(fp.clone());
            if c.inv.has_desktop(fp) && !app.desktop_ids.contains(fp) {
                app.desktop_ids.push(fp.clone());
            }
        }
    }
    c.app(app);
    let procs: Vec<String> = a.processes.clone();

    for s in &f.setting {
        // Candidate paths: native, then flatpak copies.
        let mut cands: Vec<(std::path::PathBuf, &str)> = s.files.iter().map(|p| (c.env.expand(p), "")).collect();
        if let (Some(fp), true) = (&a.flatpak, by_flatpak) {
            for p in &s.files {
                if let Some(rest) = p.strip_prefix("$XDG_CONFIG_HOME") {
                    let alt = c.env.home.join(".var/app").join(fp).join("config").join(rest.trim_start_matches('/'));
                    cands.push((alt, "Flatpak"));
                }
            }
        }
        let existing: Vec<_> = cands.iter().filter(|(p, _)| p.exists()).cloned().collect();
        let chosen = if existing.is_empty() {
            // Create the file only where the app is installed that way.
            let pick = if by_desktop || by_bin { cands.first() } else { cands.iter().find(|(_, l)| !l.is_empty()) };
            pick.cloned().into_iter().collect()
        } else {
            existing
        };
        let multi = chosen.len() > 1;
        for (path, label) in chosen {
            let target = match s.format.as_str() {
                "ini" => Target::Ini { path: path.clone(), group: s.group.clone(), key: s.key.clone(), kde_notify: false },
                "json" => Target::Json { path: path.clone(), pointer: s.pointer.clone() },
                "xcu" => Target::Xcu { path: path.clone(), item: s.item.clone(), prop: s.key.clone() },
                "gimprc" => Target::GimpRc { path: path.clone(), key: s.key.clone() },
                "xmlsetting" => Target::XmlSetting { path: path.clone(), name: s.key.clone() },
                other => {
                    eprintln!("privacy-panel: {}: unknown format {other}", a.id);
                    continue;
                }
            };
            let mut expl = s.explanation.clone();
            if let Some(n) = &s.note {
                expl = format!("{expl} {n}");
            }
            if s.requires_closed {
                expl = format!("{expl} Close {} before changing this.", a.name);
            }
            let id = if multi || !label.is_empty() {
                format!("{}.{}@{}", a.id, s.id, path.display())
            } else {
                format!("{}.{}", a.id, s.id)
            };
            let base = setting(id, &a.id, a.section, s.category, s.title.clone(), expl);
            let mut st = match s.kind.as_str() {
                "bool" => {
                    let on = s.on.clone().unwrap_or_else(|| "true".into());
                    let off = s.off.clone().unwrap_or_else(|| "false".into());
                    base.boolean(
                        target,
                        ValueMap::bool(&on, &off),
                        s.default.as_ref().and_then(|v| v.as_bool()),
                        s.recommended.as_ref().and_then(|v| v.as_bool()),
                    )
                }
                "choice" => {
                    let opts: Vec<(&str, &str, Option<&str>)> =
                        s.options.iter().map(|o| (o.id.as_str(), o.label.as_str(), o.raw.as_deref())).collect();
                    base.choice(
                        target,
                        &opts,
                        s.default.as_ref().and_then(|v| v.as_str()),
                        s.recommended.as_ref().and_then(|v| v.as_str()),
                    )
                }
                other => {
                    eprintln!("privacy-panel: {}: unknown type {other}", a.id);
                    continue;
                }
            };
            if !label.is_empty() {
                st = st.group(label);
            }
            if s.about.is_some() {
                st.about = s.about.clone();
            }
            if s.requires_closed {
                st = st.closed(&procs);
            }
            c.add(st);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_files_parse() {
        for (name, text) in BUILTIN {
            let f = parse(text).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!f.setting.is_empty(), "{name} has no settings");
            for s in &f.setting {
                assert!(["ini", "json", "xcu", "gimprc", "xmlsetting"].contains(&s.format.as_str()), "{name}/{}", s.id);
                assert!(["bool", "choice"].contains(&s.kind.as_str()), "{name}/{}", s.id);
                if s.kind == "choice" {
                    let d = s.default.as_ref().and_then(|v| v.as_str());
                    if let Some(d) = d {
                        assert!(s.options.iter().any(|o| o.id == d), "{name}/{} default", s.id);
                    }
                }
                for t in [&s.title, &s.explanation] {
                    assert!(!t.contains('\u{2014}') && !t.contains('\u{2013}'), "{name}/{}: long dash", s.id);
                }
            }
        }
    }
}
