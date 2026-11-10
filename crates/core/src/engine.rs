//! Reading current values, applying changes, undo.

use crate::admin::{AdminOp, AdminRequest, AdminResponse, AdminTarget};
use crate::env::Env;
use crate::history::{self, Entry};
use crate::model::{ActionKind, Current, Kind, Model, Setting, Value};
use crate::target::{Snapshot, Target};

/// Runs root operations. The GUI uses pkexec; tests use a fake.
pub trait AdminExecutor {
    fn run(&self, req: &AdminRequest) -> Result<AdminResponse, String>;
}

/// Refuses everything (for contexts without a helper).
pub struct NoAdmin;
impl AdminExecutor for NoAdmin {
    fn run(&self, _req: &AdminRequest) -> Result<AdminResponse, String> {
        Err("Administrator changes are not available here.".into())
    }
}

pub fn read_current(env: &Env, s: &Setting) -> Current {
    match &s.kind {
        Kind::Info | Kind::Action(_) => return s.current.clone(),
        Kind::Bool | Kind::Choice(_) => {}
    }
    let spec = &s.spec;
    let raw = match spec.target.read_raw(env) {
        Ok(r) => r,
        Err(e) => return Current::Unavailable(format!("Could not read: {e}")),
    };
    match raw {
        Some(r) => {
            let n = spec.target.normalize(&r);
            for (v, er) in &spec.map.entries {
                if let Some(er) = er {
                    if spec.target.normalize(er) == n {
                        return Current::Value { value: v.clone(), is_default: false };
                    }
                }
            }
            for (a, v) in &spec.map.aliases {
                if spec.target.normalize(a) == n {
                    return Current::Value { value: v.clone(), is_default: false };
                }
            }
            Current::Other(r)
        }
        None => {
            if let Some((v, _)) = spec.map.entries.iter().find(|(_, r)| r.is_none()) {
                return Current::Value { value: v.clone(), is_default: true };
            }
            match &spec.default {
                Some(v) => Current::Value { value: v.clone(), is_default: true },
                None => Current::Unavailable("Not set".into()),
            }
        }
    }
}

/// Refresh every setting's current value and running-app block.
pub fn refresh(env: &Env, model: &mut Model) {
    let running = crate::discovery::running_processes(env);
    let names: Vec<(String, String)> = model.apps.iter().map(|a| (a.id.clone(), a.name.clone())).collect();
    for s in &mut model.settings {
        s.current = read_current(env, s);
        s.blocked_by = if s.requires_closed.iter().any(|p| running.contains(p)) {
            names
                .iter()
                .find(|(id, _)| *id == s.app_id)
                .map(|(_, n)| n.clone())
                .or_else(|| Some(s.app_id.clone()))
        } else {
            None
        };
    }
}

#[derive(Clone, Debug)]
pub enum ChangeKind {
    Set(Option<String>),
    Restore(Snapshot),
}

#[derive(Clone, Debug)]
pub struct PlannedChange {
    pub setting_id: String,
    pub title: String,
    pub app_name: String,
    pub target: Target,
    pub kind: ChangeKind,
}

pub fn plan(model: &Model, s: &Setting, v: &Value) -> Result<PlannedChange, String> {
    if let Some(app) = &s.blocked_by {
        return Err(format!("Close {app} first."));
    }
    let raw = s
        .spec
        .map
        .raw_for(v)
        .ok_or_else(|| format!("\"{}\" cannot be set to {}", s.title, s.value_label(v)))?;
    Ok(PlannedChange {
        setting_id: s.id.clone(),
        title: s.title.clone(),
        app_name: model.app(&s.app_id).map(|a| a.name.clone()).unwrap_or_default(),
        target: s.spec.target.clone(),
        kind: ChangeKind::Set(raw),
    })
}

/// All changes that would bring settings to their recommended values.
pub fn plan_recommended<'a>(model: &Model, settings: impl Iterator<Item = &'a Setting>) -> Vec<PlannedChange> {
    settings
        .filter(|s| s.is_editable() && s.differs_from_recommended())
        .filter_map(|s| plan(model, s, s.recommended.as_ref()?).ok())
        .collect()
}

#[derive(Debug, Default)]
pub struct Report {
    pub applied: Vec<String>,
    pub errors: Vec<(String, String)>,
    pub batch: u64,
}

fn admin_targets(t: &Target) -> Vec<AdminTarget> {
    match t {
        Target::Admin(a) => vec![a.clone()],
        Target::Multi(v) => v.iter().flat_map(admin_targets).collect(),
        _ => vec![],
    }
}

fn admin_ops(t: &Target, kind: &ChangeKind) -> Vec<AdminOp> {
    match (t, kind) {
        (_, ChangeKind::Set(raw)) => admin_targets(t)
            .into_iter()
            .map(|target| AdminOp { target, raw: raw.clone() })
            .collect(),
        (Target::Admin(a), ChangeKind::Restore(Snapshot::Raw(r))) => {
            vec![AdminOp { target: a.clone(), raw: r.clone() }]
        }
        (Target::Multi(v), ChangeKind::Restore(Snapshot::Multi(s))) => v
            .iter()
            .zip(s)
            .flat_map(|(t, s)| admin_ops(t, &ChangeKind::Restore(s.clone())))
            .collect(),
        _ => vec![],
    }
}

/// Apply changes as one batch. User-level changes are written directly; root changes are
/// sent to the helper in a single request (one password prompt).
pub fn apply(env: &Env, changes: &[PlannedChange], admin: &dyn AdminExecutor, undo_of: Option<u64>) -> Report {
    let batch = history::new_batch_id(env);
    let mut report = Report { batch, ..Default::default() };
    let mut entries = Vec::new();
    let time = history::now();

    for c in changes {
        for f in c.target.files() {
            history::backup_file(env, batch, &f);
        }
    }

    let mut admin_idx: Vec<(usize, std::ops::Range<usize>)> = Vec::new();
    let mut ops: Vec<AdminOp> = Vec::new();
    let mut befores: Vec<Option<Snapshot>> = Vec::new();

    for (i, c) in changes.iter().enumerate() {
        let before = match c.target.snapshot(env) {
            Ok(b) => b,
            Err(e) => {
                report.errors.push((c.title.clone(), e));
                befores.push(None);
                continue;
            }
        };
        befores.push(Some(before.clone()));
        if c.target.is_admin() {
            let o = admin_ops(&c.target, &c.kind);
            let start = ops.len();
            ops.extend(o);
            admin_idx.push((i, start..ops.len()));
            continue;
        }
        let res = match &c.kind {
            ChangeKind::Set(raw) => c.target.write_raw(env, raw.as_deref()),
            ChangeKind::Restore(snap) => c.target.restore(env, snap),
        };
        match res.and_then(|_| c.target.snapshot(env)) {
            Ok(after) => {
                report.applied.push(c.setting_id.clone());
                entries.push(Entry {
                    batch,
                    time,
                    setting_id: c.setting_id.clone(),
                    title: c.title.clone(),
                    app_name: c.app_name.clone(),
                    target: c.target.clone(),
                    before,
                    after,
                    undo_of,
                });
            }
            Err(e) => {
                // Put back anything half written (e.g. user.js done, prefs.js failed).
                if let Err(e2) = c.target.restore(env, &before) {
                    report.errors.push((c.title.clone(), format!("{e}; could not put the old value back: {e2}")));
                } else {
                    report.errors.push((c.title.clone(), e));
                }
            }
        }
    }

    if !ops.is_empty() {
        let req = AdminRequest { ops, dry_run: false };
        match admin.run(&req) {
            Ok(resp) => {
                for (i, range) in admin_idx {
                    let c = &changes[i];
                    let Some(results) = resp.results.get(range.clone()) else {
                        report.errors.push((c.title.clone(), "The helper gave no answer.".into()));
                        continue;
                    };
                    let failed: Vec<String> = results.iter().filter(|r| !r.ok).map(|r| r.message.clone()).collect();
                    if failed.is_empty() {
                        report.applied.push(c.setting_id.clone());
                        if let (Some(before), Ok(after)) = (befores[i].clone(), c.target.snapshot(env)) {
                            entries.push(Entry {
                                batch,
                                time,
                                setting_id: c.setting_id.clone(),
                                title: c.title.clone(),
                                app_name: c.app_name.clone(),
                                target: c.target.clone(),
                                before,
                                after,
                                undo_of,
                            });
                        }
                    } else {
                        report.errors.push((c.title.clone(), failed.join("; ")));
                    }
                }
            }
            Err(e) => {
                for (i, _) in admin_idx {
                    report.errors.push((changes[i].title.clone(), e.clone()));
                }
            }
        }
    }

    if let Err(e) = history::append(env, &entries) {
        report.errors.push(("History".into(), e));
    }
    report
}

/// Undo one batch by restoring each "before" state, in reverse order.
pub fn undo_batch(env: &Env, batch: u64, admin: &dyn AdminExecutor) -> Report {
    let b = history::batches(env).into_iter().find(|b| b.id == batch);
    let Some(b) = b else {
        return Report { errors: vec![("Undo".into(), "That change is no longer in the history.".into())], ..Default::default() };
    };
    let changes: Vec<PlannedChange> = b
        .entries
        .iter()
        .rev()
        .map(|e| PlannedChange {
            setting_id: e.setting_id.clone(),
            title: e.title.clone(),
            app_name: e.app_name.clone(),
            target: e.target.clone(),
            kind: ChangeKind::Restore(e.before.clone()),
        })
        .collect();
    apply(env, &changes, admin, Some(batch))
}

/// Put every setting this program ever changed back to how it was before the first change.
pub fn restore_everything(env: &Env, admin: &dyn AdminExecutor) -> Report {
    let changes: Vec<PlannedChange> = history::original_states(env)
        .into_iter()
        .map(|e| PlannedChange {
            setting_id: e.setting_id,
            title: e.title,
            app_name: e.app_name,
            target: e.target,
            kind: ChangeKind::Restore(e.before),
        })
        .collect();
    apply(env, &changes, admin, Some(0))
}

/// One-off actions. They cannot be undone.
pub fn run_action(env: &Env, kind: ActionKind) -> Result<String, String> {
    match kind {
        ActionKind::ClearThumbnails => {
            let dir = env.cache.join("thumbnails");
            let mut n = 0u64;
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for sub in rd.flatten() {
                    if let Ok(files) = std::fs::read_dir(sub.path()) {
                        for f in files.flatten() {
                            if f.path().is_file() && std::fs::remove_file(f.path()).is_ok() {
                                n += 1;
                            }
                        }
                    }
                }
            }
            Ok(format!("Removed {n} thumbnails."))
        }
        ActionKind::ClearRecentFiles => {
            let xbel = env.data.join("recently-used.xbel");
            if xbel.exists() {
                crate::target::write_file(
                    &xbel,
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xbel version=\"1.0\"\n      xmlns:bookmark=\"http://www.freedesktop.org/standards/desktop-bookmarks\"\n      xmlns:mime=\"http://www.freedesktop.org/standards/shared-mime-info\"\n>\n</xbel>\n",
                )?;
            }
            let mut n = 0;
            if let Ok(rd) = std::fs::read_dir(env.data.join("RecentDocuments")) {
                for f in rd.flatten() {
                    if std::fs::remove_file(f.path()).is_ok() {
                        n += 1;
                    }
                }
            }
            Ok(format!("Cleared the recent files list{}.", if n > 0 { format!(" and {n} KDE entries") } else { String::new() }))
        }
    }
}
