//! INI-style files (KConfig, QSettings, GLib keyfiles, vlcrc, systemd drop-ins, shell
//! `KEY=value` files). Edits touch only the one line concerned; every other byte stays.
//!
//! The group "" means the lines before the first `[group]` header.

fn header_name(line: &str) -> Option<&str> {
    let t = line.trim();
    if !t.starts_with('[') {
        return None;
    }
    // vlcrc writes "[qt] # Qt interface".
    let t = match t.find(" #") {
        Some(i) => t[..i].trim_end(),
        None => t,
    };
    if t.ends_with(']') && t.len() >= 2 {
        Some(&t[1..t.len() - 1])
    } else {
        None
    }
}

/// Returns the value part if `line` assigns `key`. Handles KDE markers like `key[$e]=`.
fn key_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let t = line.trim_start();
    if t.starts_with('#') || t.starts_with(';') {
        return None;
    }
    let eq = t.find('=')?;
    let mut k = t[..eq].trim_end();
    if let Some(i) = k.find("[$") {
        k = &k[..i];
    }
    if k == key {
        Some(t[eq + 1..].trim())
    } else {
        None
    }
}

/// True if `line` is a commented-out assignment of `key`, as vlcrc writes defaults.
fn commented_key(line: &str, key: &str) -> bool {
    let t = line.trim_start();
    let Some(rest) = t.strip_prefix('#') else {
        return false;
    };
    match rest.find('=') {
        Some(eq) => rest[..eq].trim() == key,
        None => false,
    }
}

/// (start, end) line index ranges of each occurrence of the group, header excluded.
fn group_ranges(lines: &[&str], group: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut cur: Option<usize> = if group.is_empty() { Some(0) } else { None };
    for (i, l) in lines.iter().enumerate() {
        if let Some(name) = header_name(l) {
            if let Some(s) = cur.take() {
                out.push((s, i));
            }
            if name == group {
                cur = Some(i + 1);
            }
        }
    }
    if let Some(s) = cur {
        out.push((s, lines.len()));
    }
    out
}

pub fn get(text: &str, group: &str, key: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = None;
    for (s, e) in group_ranges(&lines, group) {
        for l in &lines[s..e] {
            if let Some(v) = key_value(l, key) {
                found = Some(v.to_string());
            }
        }
    }
    found
}

/// Set (`Some`) or remove (`None`) a key. Returns the new text.
pub fn set(text: &str, group: &str, key: &str, value: Option<&str>) -> String {
    let had_newline = text.is_empty() || text.ends_with('\n');
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let refs: Vec<&str> = text.lines().collect();
    let ranges = group_ranges(&refs, group);

    // Existing assignments of the key, in all copies of the group.
    let mut hits: Vec<usize> = Vec::new();
    let mut commented: Option<usize> = None;
    for &(s, e) in &ranges {
        for (i, l) in refs.iter().enumerate().take(e).skip(s) {
            if key_value(l, key).is_some() {
                hits.push(i);
            } else if commented.is_none() && commented_key(l, key) {
                commented = Some(i);
            }
        }
    }

    match value {
        None => {
            let removed = !hits.is_empty();
            for i in hits.into_iter().rev() {
                lines.remove(i);
            }
            // Drop a group that is now empty, with the blank line before it.
            if removed && !group.is_empty() {
                let refs2: Vec<&str> = lines.iter().map(String::as_str).collect();
                let ranges2 = group_ranges(&refs2, group);
                if ranges2.len() == 1 {
                    let (s, e) = ranges2[0];
                    if refs2[s..e].iter().all(|l| l.trim().is_empty()) {
                        let mut start = s - 1;
                        if start > 0 && lines[start - 1].trim().is_empty() {
                            start -= 1;
                        }
                        lines.drain(start..e);
                    }
                }
            }
        }
        Some(v) => {
            let new_line = format!("{key}={v}");
            if let Some((&last, rest)) = hits.split_last() {
                let indent: String = lines[last].chars().take_while(|c| c.is_whitespace()).collect();
                lines[last] = format!("{indent}{new_line}");
                for &i in rest.iter().rev() {
                    lines.remove(i);
                }
            } else if let Some(c) = commented {
                lines.insert(c + 1, new_line);
            } else if let Some(&(s, e)) = ranges.last() {
                // After the last non-empty line of the group.
                let mut at = e;
                while at > s && refs[at - 1].trim().is_empty() {
                    at -= 1;
                }
                lines.insert(at, new_line);
            } else {
                if !lines.is_empty() && !lines.last().unwrap().trim().is_empty() {
                    lines.push(String::new());
                }
                lines.push(format!("[{group}]"));
                lines.push(new_line);
            }
        }
    }

    let mut out = lines.join("\n");
    if had_newline && !out.is_empty() {
        out.push('\n');
    }
    out
}

/// All `[group]` names in order of first appearance.
pub fn groups(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in text.lines() {
        if let Some(n) = header_name(l) {
            if !out.iter().any(|g| g == n) {
                out.push(n.to_string());
            }
        }
    }
    out
}

/// All key/value pairs of a group (last assignment wins).
pub fn entries(text: &str, group: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<(String, String)> = Vec::new();
    for (s, e) in group_ranges(&lines, group) {
        for l in &lines[s..e] {
            let t = l.trim_start();
            if t.starts_with('#') || t.starts_with(';') || header_name(t).is_some() {
                continue;
            }
            if let Some(eq) = t.find('=') {
                let mut k = t[..eq].trim_end();
                if let Some(i) = k.find("[$") {
                    k = &k[..i];
                }
                let v = t[eq + 1..].trim().to_string();
                if let Some(p) = out.iter_mut().find(|(kk, _)| kk == k) {
                    p.1 = v;
                } else {
                    out.push((k.to_string(), v));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const KDE: &str = "# top comment\n[General]\ndbVersion=2\n\n[Basic Settings]\nIndexing-Enabled=true\nother = x\n\n[Last]\nk=v\n";

    #[test]
    fn get_values() {
        assert_eq!(get(KDE, "Basic Settings", "Indexing-Enabled").as_deref(), Some("true"));
        assert_eq!(get(KDE, "Basic Settings", "other").as_deref(), Some("x"));
        assert_eq!(get(KDE, "General", "Indexing-Enabled"), None);
        assert_eq!(get(KDE, "Nope", "k"), None);
    }

    #[test]
    fn replace_keeps_everything_else() {
        let out = set(KDE, "Basic Settings", "Indexing-Enabled", Some("false"));
        assert_eq!(out, KDE.replace("Indexing-Enabled=true", "Indexing-Enabled=false"));
    }

    #[test]
    fn insert_into_group_before_blank() {
        let out = set(KDE, "General", "new", Some("1"));
        assert_eq!(out, KDE.replace("dbVersion=2\n", "dbVersion=2\nnew=1\n"));
    }

    #[test]
    fn insert_new_group_and_remove() {
        let out = set(KDE, "Fresh", "a", Some("b"));
        assert!(out.ends_with("k=v\n\n[Fresh]\na=b\n"));
        assert_eq!(set(&out, "Fresh", "a", None), KDE);
    }

    #[test]
    fn remove_restores_original_bytes() {
        let added = set(KDE, "Last", "x", Some("1"));
        assert_eq!(set(&added, "Last", "x", None), KDE);
    }

    #[test]
    fn vlc_commented_defaults_and_headers() {
        let vlc = "[qt] # Qt interface\n\n# Show recently played\n#qt-recentplay=1\n\nqt-privacy-ask=0\n[core] # core\n";
        assert_eq!(get(vlc, "qt", "qt-recentplay"), None);
        assert_eq!(get(vlc, "qt", "qt-privacy-ask").as_deref(), Some("0"));
        let out = set(vlc, "qt", "qt-recentplay", Some("0"));
        assert!(out.contains("#qt-recentplay=1\nqt-recentplay=0\n"));
        assert_eq!(set(&out, "qt", "qt-recentplay", None), vlc);
    }

    #[test]
    fn kde_markers_and_top_group() {
        let t = "PARTICIPATE=\"yes\"\n[A]\nkey[$e]=$HOME/x\n";
        assert_eq!(get(t, "", "PARTICIPATE").as_deref(), Some("\"yes\""));
        assert_eq!(get(t, "A", "key").as_deref(), Some("$HOME/x"));
        assert_eq!(set(t, "", "PARTICIPATE", Some("\"no\"")), t.replace("yes", "no"));
    }

    #[test]
    fn empty_file() {
        assert_eq!(set("", "G", "k", Some("v")), "[G]\nk=v\n");
        assert_eq!(set("", "", "k", Some("v")), "k=v\n");
    }

    #[test]
    fn listing() {
        assert_eq!(groups(KDE), vec!["General", "Basic Settings", "Last"]);
        assert_eq!(entries(KDE, "Basic Settings").len(), 2);
    }
}
