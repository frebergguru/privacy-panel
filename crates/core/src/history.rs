//! Every change is recorded with the state before and after, so it can be undone.
//! Stored as one JSON object per line in ~/.local/state/privacy-panel/history.jsonl.

use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::env::Env;
use crate::target::{Snapshot, Target};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub batch: u64,
    pub time: u64,
    pub setting_id: String,
    pub title: String,
    pub app_name: String,
    pub target: Target,
    pub before: Snapshot,
    pub after: Snapshot,
    /// Set when this batch undid an earlier one.
    #[serde(default)]
    pub undo_of: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Batch {
    pub id: u64,
    pub time: u64,
    pub undo_of: Option<u64>,
    pub entries: Vec<Entry>,
}

pub fn path(env: &Env) -> PathBuf {
    env.app_state().join("history.jsonl")
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A new batch id, unique even for several batches in the same second.
pub fn new_batch_id(env: &Env) -> u64 {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    let last = load(env).iter().map(|e| e.batch).max().unwrap_or(0);
    t.max(last + 1)
}

pub fn append(env: &Env, entries: &[Entry]) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }
    let p = path(env);
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)
        .map_err(|e| format!("{}: {e}", p.display()))?;
    for e in entries {
        let line = serde_json::to_string(e).map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn load(env: &Env) -> Vec<Entry> {
    std::fs::read_to_string(path(env))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Batches, newest first.
pub fn batches(env: &Env) -> Vec<Batch> {
    let mut out: Vec<Batch> = Vec::new();
    for e in load(env) {
        match out.iter_mut().find(|b| b.id == e.batch) {
            Some(b) => b.entries.push(e),
            None => out.push(Batch { id: e.batch, time: e.time, undo_of: e.undo_of, entries: vec![e] }),
        }
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

/// Ids of batches that were already undone.
pub fn undone_ids(env: &Env) -> Vec<u64> {
    batches(env).iter().filter_map(|b| b.undo_of).collect()
}

/// For "restore everything": the earliest recorded state of each target, latest target first.
pub fn original_states(env: &Env) -> Vec<Entry> {
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<Entry> = Vec::new();
    for e in load(env) {
        let key = serde_json::to_string(&e.target).unwrap_or_default();
        if !seen.contains(&key) {
            seen.push(key);
            out.push(e);
        }
    }
    out.reverse();
    out
}

/// Copy a file into the backup folder for a batch, once.
pub fn backup_file(env: &Env, batch: u64, file: &std::path::Path) {
    if !file.exists() {
        return;
    }
    let name = file.to_string_lossy().trim_start_matches('/').replace('/', "%");
    let dir = env.app_state().join("backups").join(batch.to_string());
    let dest = dir.join(name);
    if dest.exists() {
        return;
    }
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::copy(file, dest);
}
