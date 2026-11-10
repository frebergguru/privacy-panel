//! Sends root operations to privacy-panel-helper through pkexec (one password prompt
//! per batch). With PRIVACY_PANEL_ADMIN_DRY_RUN=1 the helper runs without pkexec in
//! dry-run mode, which changes nothing; this is for testing.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use privacy_core::admin::{AdminRequest, AdminResponse};
use privacy_core::engine::AdminExecutor;

pub struct PkexecHelper;

/// Inside an AppImage the helper sits on a FUSE mount that root cannot read, so pkexec
/// could not start it from there. Copy it to a folder only this user can write to and
/// run that copy. pkexec still asks for the administrator password before running it.
fn appimage_helper() -> Option<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let appdir = std::env::var_os("APPDIR")?;
    std::env::var_os("APPIMAGE")?;
    let src = PathBuf::from(appdir).join("helper/privacy-panel-helper");
    if !src.is_file() {
        return None;
    }
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let dir = base.join("privacy-panel");
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir).ok()?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).ok()?;
    let dst = dir.join("privacy-panel-helper");
    // Write to a new file and rename it into place, so a half-written or replaced
    // file is never run.
    let tmp = dir.join(format!(".helper-{}", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    std::fs::copy(&src, &tmp).ok()?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o700)).ok()?;
    std::fs::rename(&tmp, &dst).ok()?;
    Some(dst)
}

pub fn helper_path() -> Option<PathBuf> {
    // The AppImage's own helper first, so it always matches this version of the app.
    if let Some(p) = appimage_helper() {
        return Some(p);
    }
    let mut c: Vec<PathBuf> = vec![
        PathBuf::from("/usr/lib/privacy-panel/privacy-panel-helper"),
        PathBuf::from("/usr/local/lib/privacy-panel/privacy-panel-helper"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            c.push(dir.join("privacy-panel-helper"));
        }
    }
    c.into_iter().find(|p| p.is_file())
}

fn dry_run() -> bool {
    std::env::var("PRIVACY_PANEL_ADMIN_DRY_RUN").is_ok_and(|v| v == "1")
}

impl AdminExecutor for PkexecHelper {
    fn run(&self, req: &AdminRequest) -> Result<AdminResponse, String> {
        let helper = helper_path().ok_or("The administrator helper (privacy-panel-helper) is not installed.")?;
        let mut cmd = if dry_run() {
            let mut c = Command::new(&helper);
            c.arg("--dry-run");
            c
        } else {
            let mut c = Command::new("pkexec");
            c.arg(&helper);
            c
        };
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not start pkexec: {e}"))?;
        let body = serde_json::to_string(req).map_err(|e| e.to_string())?;
        child
            .stdin
            .take()
            .ok_or("no stdin")?
            .write_all(body.as_bytes())
            .map_err(|e| e.to_string())?;
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        match out.status.code() {
            Some(126) => return Err("The administrator password was not given.".into()),
            Some(127) => return Err("You are not allowed to change system settings.".into()),
            _ => {}
        }
        let text = String::from_utf8_lossy(&out.stdout);
        serde_json::from_str(text.trim()).map_err(|_| {
            let err = String::from_utf8_lossy(&out.stderr);
            format!("The helper failed: {}", err.trim())
        })
    }
}
