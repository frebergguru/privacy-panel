//! Flatpak sandbox permissions: what an app asks for in its metadata, and the user's
//! overrides in ~/.local/share/flatpak/overrides/<app-id> (same format `flatpak override` writes).

use std::path::PathBuf;

use crate::backends::ini;
use crate::env::Env;

#[derive(Clone, Debug)]
pub struct FlatpakApp {
    pub id: String,
    pub metadata: PathBuf,
    pub user_install: bool,
}

pub fn installation_dirs(env: &Env) -> Vec<(PathBuf, bool)> {
    vec![
        (env.data.join("flatpak"), true),
        (env.sys("/var/lib/flatpak"), false),
    ]
}

pub fn installed_apps(env: &Env) -> Vec<FlatpakApp> {
    let mut out: Vec<FlatpakApp> = Vec::new();
    for (dir, user) in installation_dirs(env) {
        let Ok(rd) = std::fs::read_dir(dir.join("app")) else {
            continue;
        };
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().to_string();
            let md = e.path().join("current/active/metadata");
            if md.exists() && !out.iter().any(|a| a.id == id) {
                out.push(FlatpakApp { id, metadata: md, user_install: user });
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

pub fn user_override_path(env: &Env, app: &str) -> PathBuf {
    env.data.join("flatpak/overrides").join(app)
}

fn override_paths_before_user(env: &Env, app: &str) -> Vec<PathBuf> {
    vec![
        env.sys("/var/lib/flatpak/overrides/global"),
        env.sys("/var/lib/flatpak/overrides").join(app),
        env.data.join("flatpak/overrides/global"),
    ]
}

fn read(p: &PathBuf) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

pub fn split_list(v: &str) -> Vec<String> {
    v.split(';').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

/// Token name without access suffix: "home:ro" -> "home".
fn base(tok: &str) -> &str {
    tok.trim_start_matches('!').split(':').next().unwrap_or(tok)
}

fn apply_list(set: &mut Vec<String>, list: &[String]) {
    for t in list {
        let b = base(t).to_string();
        set.retain(|x| base(x) != b);
        if !t.starts_with('!') {
            set.push(t.clone());
        }
    }
}

/// Permission tokens for `key` ("shared", "sockets", "devices", "filesystems", "features")
/// from the metadata only.
pub fn static_tokens(md_text: &str, key: &str) -> Vec<String> {
    ini::get(md_text, "Context", key).map(|v| split_list(&v)).unwrap_or_default()
}

/// Effective tokens without the user's per-app override.
pub fn tokens_without_user(env: &Env, app: &FlatpakApp, key: &str) -> Vec<String> {
    let mut set = static_tokens(&read(&app.metadata), key);
    for p in override_paths_before_user(env, &app.id) {
        if let Some(v) = ini::get(&read(&p), "Context", key) {
            apply_list(&mut set, &split_list(&v));
        }
    }
    set
}

pub fn effective_tokens(env: &Env, app: &FlatpakApp, key: &str) -> Vec<String> {
    let mut set = tokens_without_user(env, app, key);
    if let Some(v) = ini::get(&read(&user_override_path(env, &app.id)), "Context", key) {
        apply_list(&mut set, &split_list(&v));
    }
    set
}

/// The user override entry for one token: "tok", "!tok" or None.
pub fn user_entry(env: &Env, app: &str, key: &str, token: &str) -> Option<String> {
    let v = ini::get(&read(&user_override_path(env, app)), "Context", key)?;
    split_list(&v).into_iter().find(|t| base(t) == token)
}

/// Replace the user override entry for one token. `None` removes it.
pub fn set_user_entry(env: &Env, app: &str, key: &str, token: &str, entry: Option<&str>) -> Result<(), String> {
    let path = user_override_path(env, app);
    let text = read(&path);
    let mut list = ini::get(&text, "Context", key).map(|v| split_list(&v)).unwrap_or_default();
    list.retain(|t| base(t) != token);
    if let Some(e) = entry {
        list.push(e.to_string());
    }
    let new = if list.is_empty() {
        ini::set(&text, "Context", key, None)
    } else {
        ini::set(&text, "Context", key, Some(&(list.join(";") + ";")))
    };
    crate::target::write_file(&path, &new)
}

/// "on" or "off" for whether a token is granted after all overrides.
pub fn granted(env: &Env, app: &FlatpakApp, key: &str, token: &str) -> bool {
    effective_tokens(env, app, key).iter().any(|t| base(t) == token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_logic() {
        let mut set = vec!["home".to_string(), "xdg-download:ro".into()];
        apply_list(&mut set, &["!home".into(), "host".into(), "!xdg-download".into()]);
        assert_eq!(set, vec!["host".to_string()]);
        assert_eq!(split_list("network;ipc;"), vec!["network", "ipc"]);
    }
}
