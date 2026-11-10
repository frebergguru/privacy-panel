//! Where a setting's raw value lives, and how to read, write, snapshot and restore it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::admin::AdminTarget;
use crate::backends::{flatpak, ini, jsonc, system, textfmt};
use crate::env::Env;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Target {
    /// Nothing to read or write (information rows, actions).
    None,
    Ini { path: PathBuf, group: String, key: String, kde_notify: bool },
    Json { path: PathBuf, pointer: Vec<String> },
    /// A Firefox-family pref: read from user.js then prefs.js, written to both.
    MozPref { profile: PathBuf, pref: String },
    Xcu { path: PathBuf, item: String, prop: String },
    GimpRc { path: PathBuf, key: String },
    XmlSetting { path: PathBuf, name: String },
    GSettings { schema: String, key: String },
    /// Raw value is a JSON array of permission strings.
    Portal { table: String, id: String, app: String },
    /// Raw value is "on" or "off".
    Flatpak {
        app: String,
        metadata: PathBuf,
        key: String,
        token: String,
        /// Token written when turning it on, with any access suffix such as ":ro".
        #[serde(default)]
        grant: Option<String>,
    },
    /// A snap interface plug. Raw value is "connected" or "disconnected".
    Snap { plug: String, slot: String },
    /// Several places written together; read from the first.
    Multi(Vec<Target>),
    /// Needs the root helper to write.
    Admin(AdminTarget),
}

/// The full stored state of a target, used for undo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Snapshot {
    Raw(Option<String>),
    Moz { user: Option<String>, prefs: Option<String> },
    Multi(Vec<Snapshot>),
}

pub fn read_file(p: &Path) -> Result<String, String> {
    match std::fs::read_to_string(p) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

/// Write a file atomically (temporary file in the same folder, then rename),
/// keeping the old file's permissions.
pub fn write_file(p: &Path, text: &str) -> Result<(), String> {
    // Write through symbolic links (dotfile managers), so the link itself survives.
    // The root helper refuses symbolic links before it gets here.
    let real;
    let p = match std::fs::symlink_metadata(p) {
        Ok(m) if m.file_type().is_symlink() => {
            real = std::fs::canonicalize(p).map_err(|e| format!("{}: {e}", p.display()))?;
            real.as_path()
        }
        _ => p,
    };
    let dir = p.parent().ok_or("path has no folder")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name = p.file_name().ok_or("path has no file name")?.to_string_lossy();
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(".{name}.privacy-panel.{}.{n}.tmp", std::process::id()));
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    if let Ok(meta) = std::fs::metadata(p) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, p).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", p.display())
    })
}

fn edit_file(p: &Path, f: impl FnOnce(&str) -> Result<String, String>) -> Result<(), String> {
    let old = read_file(p)?;
    let new = f(&old)?;
    if new == old {
        return Ok(());
    }
    if new.trim().is_empty() {
        // Nothing left (for example after undoing a key in a file we created).
        return match std::fs::remove_file(p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", p.display())),
            _ => Ok(()),
        };
    }
    write_file(p, &new)
}

impl Target {
    pub fn is_admin(&self) -> bool {
        match self {
            Target::Admin(_) => true,
            Target::Multi(v) => v.iter().any(Target::is_admin),
            _ => false,
        }
    }

    /// Files this target writes (for backups).
    pub fn files(&self) -> Vec<PathBuf> {
        match self {
            Target::Ini { path, .. }
            | Target::Json { path, .. }
            | Target::Xcu { path, .. }
            | Target::GimpRc { path, .. }
            | Target::XmlSetting { path, .. } => vec![path.clone()],
            Target::MozPref { profile, .. } => vec![profile.join("user.js"), profile.join("prefs.js")],
            Target::Multi(v) => v.iter().flat_map(Target::files).collect(),
            _ => vec![],
        }
    }

    /// Bring raw text to a canonical form so it can be compared with a map entry.
    pub fn normalize(&self, raw: &str) -> String {
        match self {
            Target::Json { .. } | Target::MozPref { .. } | Target::Portal { .. } => {
                serde_json::from_str::<serde_json::Value>(raw)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|_| raw.trim().to_string())
            }
            Target::Admin(AdminTarget::Json { .. }) => serde_json::from_str::<serde_json::Value>(raw)
                .map(|v| v.to_string())
                .unwrap_or_else(|_| raw.trim().to_string()),
            Target::GSettings { .. } => system::gvariant_normalize(raw),
            Target::Multi(v) => v.first().map(|t| t.normalize(raw)).unwrap_or_default(),
            _ => raw.trim().to_string(),
        }
    }

    pub fn read_raw(&self, env: &Env) -> Result<Option<String>, String> {
        match self {
            Target::None => Ok(None),
            Target::Ini { path, group, key, .. } => Ok(ini::get(&read_file(path)?, group, key)),
            Target::Json { path, pointer } => jsonc::get(&read_file(path)?, pointer),
            Target::MozPref { profile, pref } => {
                let u = read_file(&profile.join("user.js"))?;
                if let Some(v) = textfmt::moz_get(&u, pref) {
                    return Ok(Some(v));
                }
                Ok(textfmt::moz_get(&read_file(&profile.join("prefs.js"))?, pref))
            }
            Target::Xcu { path, item, prop } => Ok(textfmt::xcu_get(&read_file(path)?, item, prop)),
            Target::GimpRc { path, key } => Ok(textfmt::gimprc_get(&read_file(path)?, key)),
            Target::XmlSetting { path, name } => Ok(textfmt::xmlsetting_get(&read_file(path)?, name)),
            Target::GSettings { schema, key } => {
                if !env.live {
                    return Err("gsettings is not available".into());
                }
                system::gsettings_get(schema, key).map(Some)
            }
            Target::Portal { table, id, app } => {
                if !env.live {
                    return Err("the permission store is not available".into());
                }
                let perms = system::portal_lookup(table, id)?;
                Ok(perms.get(app).map(|v| serde_json::to_string(v).unwrap()))
            }
            Target::Flatpak { app, metadata, key, token, .. } => {
                let fa = flatpak::FlatpakApp { id: app.clone(), metadata: metadata.clone(), user_install: false };
                Ok(Some(if flatpak::granted(env, &fa, key, token) { "on" } else { "off" }.into()))
            }
            Target::Snap { plug, .. } => {
                if !env.live {
                    return Err("snapd is not available".into());
                }
                system::snap_plug_state(plug).map(Some)
            }
            Target::Multi(v) => match v.first() {
                Some(t) => t.read_raw(env),
                None => Ok(None),
            },
            Target::Admin(a) => a.read(env),
        }
    }

    /// Write a raw value (`None` removes it). Admin targets are refused here; they go
    /// through the root helper.
    pub fn write_raw(&self, env: &Env, raw: Option<&str>) -> Result<(), String> {
        match self {
            Target::None => Ok(()),
            Target::Ini { path, group, key, kde_notify } => {
                edit_file(path, |t| Ok(ini::set(t, group, key, raw)))?;
                if *kde_notify && env.live {
                    if let Some(name) = path.file_name() {
                        system::kconfig_notify(&name.to_string_lossy(), group, key);
                    }
                }
                Ok(())
            }
            Target::Json { path, pointer } => edit_file(path, |t| jsonc::set(t, pointer, raw)),
            Target::MozPref { profile, pref } => {
                edit_file(&profile.join("user.js"), |t| Ok(textfmt::moz_block_set(t, pref, raw)))?;
                edit_file(&profile.join("prefs.js"), |t| Ok(textfmt::moz_set(t, pref, raw)))
            }
            Target::Xcu { path, item, prop } => {
                if !path.exists() {
                    return Err("LibreOffice has not created its settings file yet. Start it once first.".into());
                }
                edit_file(path, |t| textfmt::xcu_set(t, item, prop, raw))
            }
            Target::GimpRc { path, key } => edit_file(path, |t| Ok(textfmt::gimprc_set(t, key, raw))),
            Target::XmlSetting { path, name } => {
                if !path.exists() {
                    return Err("The settings file does not exist yet. Start the app once first.".into());
                }
                edit_file(path, |t| textfmt::xmlsetting_set(t, name, raw))
            }
            Target::GSettings { schema, key } => {
                if !env.live {
                    return Err("gsettings is not available".into());
                }
                system::gsettings_set(schema, key, raw)
            }
            Target::Portal { table, id, app } => {
                if !env.live {
                    return Err("the permission store is not available".into());
                }
                match raw {
                    Some(r) => {
                        let v: Vec<String> = serde_json::from_str(r).map_err(|e| e.to_string())?;
                        system::portal_set(table, id, app, Some(&v))
                    }
                    None => system::portal_set(table, id, app, None),
                }
            }
            Target::Flatpak { app, metadata, key, token, grant } => {
                let fa = flatpak::FlatpakApp { id: app.clone(), metadata: metadata.clone(), user_install: false };
                let base_on = flatpak::tokens_without_user(env, &fa, key)
                    .iter()
                    .any(|t| t.split(':').next() == Some(token.as_str()));
                let entry = match raw {
                    None => None,
                    Some("on") if base_on => None,
                    Some("off") if !base_on => None,
                    Some("on") => Some(grant.clone().unwrap_or_else(|| token.clone())),
                    Some("off") => Some(format!("!{token}")),
                    Some(other) => return Err(format!("bad flatpak value {other}")),
                };
                flatpak::set_user_entry(env, app, key, token, entry.as_deref())
            }
            Target::Snap { plug, slot } => {
                if !env.live {
                    return Err("snapd is not available".into());
                }
                system::snap_set(plug, slot, raw == Some("connected"))
            }
            Target::Multi(v) => {
                for t in v {
                    t.write_raw(env, raw)?;
                }
                Ok(())
            }
            Target::Admin(_) => Err("administrator settings must go through the helper".into()),
        }
    }

    pub fn snapshot(&self, env: &Env) -> Result<Snapshot, String> {
        match self {
            Target::MozPref { profile, pref } => Ok(Snapshot::Moz {
                user: textfmt::moz_block_get(&read_file(&profile.join("user.js"))?, pref),
                prefs: textfmt::moz_get(&read_file(&profile.join("prefs.js"))?, pref),
            }),
            Target::Flatpak { app, key, token, .. } => {
                Ok(Snapshot::Raw(flatpak::user_entry(env, app, key, token)))
            }
            Target::Multi(v) => Ok(Snapshot::Multi(
                v.iter().map(|t| t.snapshot(env)).collect::<Result<_, _>>()?,
            )),
            Target::Admin(a) => Ok(Snapshot::Raw(a.read_own(env)?)),
            _ => Ok(Snapshot::Raw(self.read_raw(env)?)),
        }
    }

    /// Put back a snapshot (user targets only).
    pub fn restore(&self, env: &Env, snap: &Snapshot) -> Result<(), String> {
        match (self, snap) {
            (Target::MozPref { profile, pref }, Snapshot::Moz { user, prefs }) => {
                edit_file(&profile.join("user.js"), |t| Ok(textfmt::moz_block_set(t, pref, user.as_deref())))?;
                edit_file(&profile.join("prefs.js"), |t| Ok(textfmt::moz_set(t, pref, prefs.as_deref())))
            }
            (Target::Flatpak { app, key, token, .. }, Snapshot::Raw(entry)) => {
                flatpak::set_user_entry(env, app, key, token, entry.as_deref())
            }
            (Target::Multi(v), Snapshot::Multi(s)) if v.len() == s.len() => {
                for (t, sn) in v.iter().zip(s) {
                    t.restore(env, sn)?;
                }
                Ok(())
            }
            (t, Snapshot::Raw(r)) => t.write_raw(env, r.as_deref()),
            _ => Err("snapshot does not match the setting".into()),
        }
    }
}
