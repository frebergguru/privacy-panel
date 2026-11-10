//! Things read through other programs or D-Bus: gsettings, systemd unit state and the
//! XDG desktop portal permission store.

use std::collections::HashMap;
use std::process::Command;

use crate::env::Env;

fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| format!("could not run {cmd}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

// ---------------------------------------------------------------- gsettings

pub fn gsettings_has_key(env: &Env, schema: &str, key: &str) -> bool {
    env.live
        && run("gsettings", &["list-keys", schema])
            .map(|o| o.lines().any(|l| l == key))
            .unwrap_or(false)
}

pub fn gsettings_get(schema: &str, key: &str) -> Result<String, String> {
    run("gsettings", &["get", schema, key])
}

pub fn gsettings_set(schema: &str, key: &str, raw: Option<&str>) -> Result<(), String> {
    match raw {
        Some(r) => run("gsettings", &["set", schema, key, r]).map(|_| ()),
        None => run("gsettings", &["reset", schema, key]).map(|_| ()),
    }
}

/// Make GVariant text comparable with our raw values: drop type prefixes like "uint32 "
/// and use double quotes for strings.
pub fn gvariant_normalize(raw: &str) -> String {
    let mut t = raw.trim();
    for p in ["uint32 ", "int32 ", "uint64 ", "int64 ", "byte ", "double "] {
        if let Some(r) = t.strip_prefix(p) {
            t = r;
        }
    }
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
        format!("\"{}\"", &t[1..t.len() - 1])
    } else {
        t.to_string()
    }
}

// ---------------------------------------------------------------- systemd

/// `systemctl is-enabled` for a system unit: "enabled", "disabled", "masked", "static"...
/// Returns None when the unit does not exist.
pub fn unit_state(env: &Env, unit: &str) -> Option<String> {
    if !env.live {
        return None;
    }
    let out = Command::new("systemctl")
        .args(["is-enabled", unit])
        .env("LC_ALL", "C")
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() || s == "not-found" {
        None
    } else {
        Some(s)
    }
}

// ---------------------------------------------------------------- portal permission store

const PS_DEST: &str = "org.freedesktop.impl.portal.PermissionStore";
const PS_PATH: &str = "/org/freedesktop/impl/portal/PermissionStore";

fn ps_proxy() -> Result<zbus::blocking::Proxy<'static>, String> {
    let conn = zbus::blocking::Connection::session().map_err(|e| e.to_string())?;
    zbus::blocking::Proxy::new(&conn, PS_DEST, PS_PATH, PS_DEST).map_err(|e| e.to_string())
}

/// All ids in a permission table.
pub fn portal_list(table: &str) -> Result<Vec<String>, String> {
    ps_proxy()?
        .call::<_, _, Vec<String>>("List", &(table,))
        .map_err(|e| e.to_string())
}

/// Permissions per app for one (table, id).
pub fn portal_lookup(table: &str, id: &str) -> Result<HashMap<String, Vec<String>>, String> {
    let (perms, _data): (HashMap<String, Vec<String>>, zbus::zvariant::OwnedValue) = ps_proxy()?
        .call("Lookup", &(table, id))
        .map_err(|e| e.to_string())?;
    Ok(perms)
}

pub fn portal_set(table: &str, id: &str, app: &str, perms: Option<&[String]>) -> Result<(), String> {
    let p = ps_proxy()?;
    match perms {
        Some(v) => p
            .call::<_, _, ()>("SetPermission", &(table, true, id, app, v))
            .map_err(|e| e.to_string()),
        None => p
            .call::<_, _, ()>("DeletePermission", &(table, id, app))
            .map_err(|e| e.to_string()),
    }
}

// ---------------------------------------------------------------- KDE config change signal

/// Tell running KDE programs that a KConfig key changed (like `kwriteconfig6 --notify`).
pub fn kconfig_notify(file_name: &str, group: &str, key: &str) {
    let Ok(conn) = zbus::blocking::Connection::session() else {
        return;
    };
    let path = format!("/{}", file_name.replace(['-', '.'], "_"));
    let mut map: HashMap<String, Vec<Vec<u8>>> = HashMap::new();
    map.insert(group.to_string(), vec![key.as_bytes().to_vec()]);
    if let Ok(p) = zbus::zvariant::ObjectPath::try_from(path.as_str()) {
        let _ = conn.emit_signal(
            None::<&str>,
            p,
            "org.kde.kconfig.notify",
            "ConfigChanged",
            &(map,),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gv() {
        assert_eq!(gvariant_normalize("uint32 30"), "30");
        assert_eq!(gvariant_normalize("'hello'"), "\"hello\"");
        assert_eq!(gvariant_normalize("true"), "true");
    }
}

// ---------------------------------------------------------------- snap interfaces

#[derive(Clone, Debug)]
pub struct SnapConn {
    pub interface: String,
    pub plug: String,
    /// Empty when not connected.
    pub slot: String,
}

/// All plugs, connected or not (`snap connections --all`).
pub fn snap_connections() -> Result<Vec<SnapConn>, String> {
    let out = run("snap", &["connections", "--all"])?;
    Ok(out
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 3 {
                return None;
            }
            Some(SnapConn {
                interface: f[0].to_string(),
                plug: f[1].to_string(),
                slot: if f[2] == "-" { String::new() } else { f[2].to_string() },
            })
        })
        .collect())
}

pub fn snap_plug_state(plug: &str) -> Result<String, String> {
    let all = snap_connections()?;
    let c = all.iter().find(|c| c.plug == plug).ok_or("plug not found")?;
    Ok(if c.slot.is_empty() { "disconnected" } else { "connected" }.into())
}

/// snapd itself asks for the administrator password through polkit.
pub fn snap_set(plug: &str, slot: &str, connect: bool) -> Result<(), String> {
    if connect {
        if slot.is_empty() {
            run("snap", &["connect", plug]).map(|_| ())
        } else {
            run("snap", &["connect", plug, slot]).map(|_| ())
        }
    } else {
        run("snap", &["disconnect", plug]).map(|_| ())
    }
}
