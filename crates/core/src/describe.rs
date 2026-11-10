//! "What changes on your computer": the exact file, key and values, or command, behind
//! each setting, in words.

use crate::admin::AdminTarget;
use crate::env::Env;
use crate::model::{Kind, Setting};
use crate::target::Target;

fn short(env: &Env, p: &std::path::Path) -> String {
    let s = p.display().to_string();
    let home = env.home.display().to_string();
    match s.strip_prefix(&home) {
        Some(rest) => format!("~{rest}"),
        None => s,
    }
}

/// One line per choice: "On: writes `1`", "Off: removes the line".
fn values(s: &Setting, fmt: &dyn Fn(Option<&str>) -> String) -> String {
    s.spec
        .map
        .entries
        .iter()
        .map(|(v, raw)| format!("{}: {}", s.value_label(v), fmt(raw.as_deref())))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn change(env: &Env, s: &Setting) -> Option<String> {
    if !matches!(s.kind, Kind::Bool | Kind::Choice(_)) {
        return None;
    }
    Some(target(env, s, &s.spec.target))
}

fn target(env: &Env, s: &Setting, t: &Target) -> String {
    match t {
        Target::None => String::new(),
        Target::Ini { path, group, key, .. } => {
            let where_ = if group.is_empty() { String::new() } else { format!(" in the [{group}] section") };
            format!(
                "Edits the line \"{key}=...\"{where_} of {}. Nothing else in the file is touched.\n{}",
                short(env, path),
                values(s, &|r| match r {
                    Some(r) => format!("writes {key}={r}"),
                    None => "removes the line, so the default applies".into(),
                })
            )
        }
        Target::Json { path, pointer } => format!(
            "Edits the value \"{}\" in {}. Nothing else in the file is touched.\n{}",
            pointer.join(" > "),
            short(env, path),
            values(s, &|r| match r {
                Some(r) => format!("sets it to {r}"),
                None => "removes it, so the default applies".into(),
            })
        ),
        Target::MozPref { profile, pref } => format!(
            "Sets the preference \"{pref}\", the same one you can see on the about:config page. It is written to user.js (in a block marked \"privacy-panel\") and prefs.js in the profile folder {}.\n{}",
            short(env, profile),
            values(s, &|r| match r {
                Some(r) => format!("{pref} = {r}"),
                None => "removed, so the default applies".into(),
            })
        ),
        Target::Xcu { path, item, prop } => format!(
            "Changes the LibreOffice option \"{prop}\" (under {item}). You can see it in Tools > Options > Advanced > Open Expert Configuration. Stored in {}.\n{}",
            short(env, path),
            values(s, &|r| match r {
                Some(r) => format!("{prop} = {r}"),
                None => "removed, so the default applies".into(),
            })
        ),
        Target::GimpRc { path, key } => format!(
            "Edits the line \"({key} ...)\" in {}.\n{}",
            short(env, path),
            values(s, &|r| match r {
                Some(r) => format!("({key} {r})"),
                None => "the line is removed, so the default applies".into(),
            })
        ),
        Target::XmlSetting { path, name } => format!(
            "Edits the setting \"{name}\" in {}.\n{}",
            short(env, path),
            values(s, &|r| match r {
                Some(r) => format!("set to {r}"),
                None => "removed".into(),
            })
        ),
        Target::GSettings { schema, key } => format!(
            "Changes the GNOME setting {schema} {key}, the same as the command \"gsettings set {schema} {key} ...\".\n{}",
            values(s, &|r| match r {
                Some(r) => format!("gsettings set {schema} {key} {r}"),
                None => format!("gsettings reset {schema} {key}"),
            })
        ),
        Target::Portal { table, id, app } => format!(
            "Changes the remembered answer for \"{app}\" in the desktop's permission store (table \"{table}\", entry \"{id}\"). Off stores a \"no\", so the app is refused or has to ask again."
        ),
        Target::Flatpak { app, key, token, .. } => {
            let opt = match key.as_str() {
                "shared" => "share",
                "sockets" => "socket",
                "devices" => "device",
                "features" => "allow",
                _ => "filesystem",
            };
            format!(
                "Adds a local override for this app in ~/.local/share/flatpak/overrides/{app}. It is the same as running:\nOff: flatpak override --user --no{opt}={token} {app}\nOn: flatpak override --user --{opt}={token} {app}\nThe app itself is not changed; deleting the override file puts everything back."
            )
        }
        Target::Snap { plug, .. } => format!(
            "Off runs \"snap disconnect {plug}\", on runs \"snap connect {plug}\". snapd asks for the administrator password."
        ),
        Target::Multi(v) => v.iter().map(|t| target(env, s, t)).collect::<Vec<_>>().join("\n\n"),
        Target::Admin(a) => admin(s, a),
    }
}

fn admin(s: &Setting, a: &AdminTarget) -> String {
    match a {
        AdminTarget::Unit { unit } => {
            if s.spec.map.raw_for(&crate::Value::Bool(false)) == Some(Some("masked".into())) {
                format!("Off runs \"systemctl mask --now {unit}\": the service is stopped and cannot be started by anything until it is turned on again.\nOn runs \"systemctl unmask {unit}\".\nNeeds the administrator password.")
            } else {
                format!("Off runs \"systemctl disable --now {unit}\": the service stops now and no longer starts when the computer boots.\nOn runs \"systemctl enable --now {unit}\".\nNeeds the administrator password.")
            }
        }
        AdminTarget::Ini { file, group, key } => {
            let own = !file.must_exist();
            let what = if own {
                format!("Writes \"{key}=...\" in the [{group}] section of {}, a file that belongs to Privacy Panel. Your distribution's own files are not changed; deleting this file undoes everything in it.", file.path())
            } else if group.is_empty() {
                format!("Edits the line \"{key}=...\" in {}.", file.path())
            } else {
                format!("Edits \"{key}=...\" in the [{group}] section of {}.", file.path())
            };
            format!(
                "{what}\n{}\nNeeds the administrator password.",
                values(s, &|r| match r {
                    Some(r) => format!("{key}={r}"),
                    None => "the line is removed, so the system default applies".into(),
                })
            )
        }
        AdminTarget::Json { file, pointer } => {
            let name = pointer.iter().skip_while(|p| *p == "policies").cloned().collect::<Vec<_>>().join(" > ");
            format!(
                "On sets the policy \"{name}\" in {}. Off removes it again. The browser reads policies when it starts.\nNeeds the administrator password.",
                file.path()
            )
        }
    }
}
