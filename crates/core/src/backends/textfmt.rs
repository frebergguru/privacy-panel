//! Small line-based formats: Mozilla prefs (prefs.js / user.js), LibreOffice
//! registrymodifications.xcu, gimprc and FileZilla's XML settings.
//! Each edit replaces, inserts or deletes a single line.

fn finish(lines: Vec<String>, had_newline: bool) -> String {
    let mut out = lines.join("\n");
    if had_newline && !out.is_empty() {
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------- Mozilla prefs

pub const MOZ_BEGIN: &str = "// BEGIN privacy-panel (managed by Privacy Panel, edits here may be replaced)";
pub const MOZ_END: &str = "// END privacy-panel";

/// Parse `user_pref("name", value);` and return (name, raw value).
pub fn moz_parse_line(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    let rest = t
        .strip_prefix("user_pref(")
        .or_else(|| t.strip_prefix("pref("))?;
    let rest = rest.trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    // Find the closing quote of the name, honouring escapes.
    let b = rest.as_bytes();
    let mut i = 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => break,
            _ => i += 1,
        }
    }
    if i >= b.len() {
        return None;
    }
    let name: String = serde_json::from_str(&rest[..=i]).ok()?;
    let after = rest[i + 1..].trim_start().strip_prefix(',')?;
    let end = after.rfind(')')?;
    Some((name, after[..end].trim().to_string()))
}

pub fn moz_get(text: &str, pref: &str) -> Option<String> {
    text.lines()
        .filter_map(moz_parse_line)
        .filter(|(n, _)| n == pref)
        .next_back()
        .map(|(_, v)| v)
}

fn moz_line(pref: &str, raw: &str) -> String {
    format!("user_pref({}, {});", serde_json::to_string(pref).unwrap(), raw)
}

/// Set or remove a pref anywhere in the file (prefs.js style).
pub fn moz_set(text: &str, pref: &str, raw: Option<&str>) -> String {
    let had_nl = text.is_empty() || text.ends_with('\n');
    let mut lines: Vec<String> = Vec::new();
    let mut done = false;
    for l in text.lines() {
        if matches!(moz_parse_line(l), Some((n, _)) if n == pref) {
            if let (Some(r), false) = (raw, done) {
                lines.push(moz_line(pref, r));
                done = true;
            }
            continue;
        }
        lines.push(l.to_string());
    }
    if let (Some(r), false) = (raw, done) {
        lines.push(moz_line(pref, r));
    }
    finish(lines, had_nl)
}

/// The value of a pref inside our managed block in user.js.
pub fn moz_block_get(text: &str, pref: &str) -> Option<String> {
    let mut inside = false;
    let mut found = None;
    for l in text.lines() {
        let t = l.trim();
        if t.starts_with("// BEGIN privacy-panel") {
            inside = true;
        } else if t == MOZ_END {
            inside = false;
        } else if inside {
            if let Some((n, v)) = moz_parse_line(l) {
                if n == pref {
                    found = Some(v);
                }
            }
        }
    }
    found
}

/// Set or remove a pref inside our managed block at the end of user.js.
/// The block is created when needed and removed when it becomes empty.
pub fn moz_block_set(text: &str, pref: &str, raw: Option<&str>) -> String {
    let mut before: Vec<String> = Vec::new();
    let mut block: Vec<String> = Vec::new();
    let mut after: Vec<String> = Vec::new();
    let mut state = 0; // 0 before, 1 inside, 2 after
    for l in text.lines() {
        let t = l.trim();
        match state {
            0 if t.starts_with("// BEGIN privacy-panel") => state = 1,
            1 if t == MOZ_END => state = 2,
            0 => before.push(l.to_string()),
            1 => block.push(l.to_string()),
            _ => after.push(l.to_string()),
        }
    }
    block.retain(|l| !matches!(moz_parse_line(l), Some((n, _)) if n == pref));
    if let Some(r) = raw {
        block.push(moz_line(pref, r));
    }
    let mut lines = before;
    lines.extend(after);
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if !block.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(MOZ_BEGIN.to_string());
        lines.extend(block);
        lines.push(MOZ_END.to_string());
    }
    finish(lines, true)
}

// ---------------------------------------------------------------- LibreOffice XCU

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn xcu_prefix(item: &str, prop: &str) -> String {
    format!(
        "<item oor:path=\"{}\"><prop oor:name=\"{}\"",
        xml_escape(item),
        xml_escape(prop)
    )
}

pub fn xcu_get(text: &str, item: &str, prop: &str) -> Option<String> {
    let pre = xcu_prefix(item, prop);
    let line = text.lines().filter(|l| l.trim_start().starts_with(&pre)).next_back()?;
    let s = line.find("<value>")? + "<value>".len();
    let e = line[s..].find("</value>")? + s;
    Some(xml_unescape(&line[s..e]))
}

pub fn xcu_set(text: &str, item: &str, prop: &str, raw: Option<&str>) -> Result<String, String> {
    let had_nl = text.ends_with('\n');
    let pre = xcu_prefix(item, prop);
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| !l.trim_start().starts_with(&pre))
        .map(String::from)
        .collect();
    if let Some(r) = raw {
        let new = format!(
            "{pre} oor:op=\"fuse\"><value>{}</value></prop></item>",
            xml_escape(r)
        );
        let close = lines
            .iter()
            .rposition(|l| l.contains("</oor:items>"))
            .ok_or("registrymodifications.xcu has no </oor:items>")?;
        let idx = lines[close].find("</oor:items>").unwrap();
        if idx == 0 {
            lines.insert(close, new);
        } else {
            // Closing tag shares a line with content: split it.
            let head = lines[close][..idx].to_string();
            let tail = lines[close][idx..].to_string();
            lines[close] = head;
            lines.insert(close + 1, new);
            lines.insert(close + 2, tail);
        }
    }
    Ok(finish(lines, had_nl))
}

// ---------------------------------------------------------------- gimprc

fn gimprc_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let t = line.trim();
    let rest = t.strip_prefix('(')?.strip_prefix(key)?;
    if !rest.starts_with(' ') {
        return None;
    }
    let end = rest.rfind(')')?;
    Some(rest[..end].trim())
}

pub fn gimprc_get(text: &str, key: &str) -> Option<String> {
    text.lines().filter_map(|l| gimprc_value(l, key)).next_back().map(String::from)
}

pub fn gimprc_set(text: &str, key: &str, raw: Option<&str>) -> String {
    let had_nl = text.is_empty() || text.ends_with('\n');
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| gimprc_value(l, key).is_none())
        .map(String::from)
        .collect();
    if let Some(r) = raw {
        // Keep the trailing "# end of gimprc" comment last.
        let pos = lines
            .iter()
            .rposition(|l| l.trim_start().starts_with("# end of"))
            .unwrap_or(lines.len());
        lines.insert(pos, format!("({key} {r})"));
    }
    finish(lines, had_nl)
}

// ---------------------------------------------------------------- FileZilla <Setting name="..">

fn xs_open(name: &str) -> String {
    format!("<Setting name=\"{}\"", xml_escape(name))
}

pub fn xmlsetting_get(text: &str, name: &str) -> Option<String> {
    let open = xs_open(name);
    let line = text.lines().find(|l| l.trim_start().starts_with(&open))?;
    let rest = line.trim_start()[open.len()..].trim_start();
    if rest.starts_with("/>") {
        return Some(String::new());
    }
    let s = rest.find('>')? + 1;
    let e = rest.find("</Setting>")?;
    Some(xml_unescape(&rest[s..e]))
}

pub fn xmlsetting_set(text: &str, name: &str, raw: Option<&str>) -> Result<String, String> {
    let had_nl = text.ends_with('\n');
    let open = xs_open(name);
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let pos = lines.iter().position(|l| l.trim_start().starts_with(&open));
    match (pos, raw) {
        (Some(i), Some(r)) => {
            let indent: String = lines[i].chars().take_while(|c| c.is_whitespace()).collect();
            lines[i] = format!("{indent}{open}>{}</Setting>", xml_escape(r));
        }
        (Some(i), None) => {
            lines.remove(i);
        }
        (None, Some(r)) => {
            let close = lines
                .iter()
                .position(|l| l.trim() == "</Settings>")
                .ok_or("no <Settings> section")?;
            let indent: String = lines
                .get(close.wrapping_sub(1))
                .map(|l| l.chars().take_while(|c| c.is_whitespace()).collect())
                .unwrap_or_default();
            lines.insert(close, format!("{indent}{open}>{}</Setting>", xml_escape(r)));
        }
        (None, None) => {}
    }
    Ok(finish(lines, had_nl))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moz_prefs() {
        let p = "// Mozilla User Preferences\nuser_pref(\"a.b\", true);\nuser_pref(\"s\", \"x, y\");\n";
        assert_eq!(moz_get(p, "a.b").as_deref(), Some("true"));
        assert_eq!(moz_get(p, "s").as_deref(), Some("\"x, y\""));
        let q = moz_set(p, "a.b", Some("false"));
        assert_eq!(q, p.replace("true", "false"));
        let r = moz_set(p, "new", Some("1"));
        assert_eq!(moz_set(&r, "new", None), p);
    }

    #[test]
    fn moz_block() {
        let u = "user_pref(\"keep\", 1);\n";
        let a = moz_block_set(u, "x", Some("false"));
        assert!(a.starts_with(u));
        assert_eq!(moz_block_get(&a, "x").as_deref(), Some("false"));
        assert_eq!(moz_block_get(&a, "keep"), None);
        assert_eq!(moz_get(&a, "x").as_deref(), Some("false"));
        let b = moz_block_set(&a, "y", Some("2"));
        let c = moz_block_set(&b, "x", Some("true"));
        assert_eq!(moz_block_get(&c, "x").as_deref(), Some("true"));
        let d = moz_block_set(&moz_block_set(&c, "x", None), "y", None);
        assert_eq!(d, u);
        assert_eq!(moz_block_set("", "z", None), "");
    }

    #[test]
    fn xcu() {
        let x = "<?xml version=\"1.0\"?>\n<oor:items xmlns:oor=\"x\">\n<item oor:path=\"/org.openoffice.Office.Common/Misc\"><prop oor:name=\"CrashReport\" oor:op=\"fuse\"><value>true</value></prop></item>\n</oor:items>\n";
        assert_eq!(xcu_get(x, "/org.openoffice.Office.Common/Misc", "CrashReport").as_deref(), Some("true"));
        let job = "/org.openoffice.Office.Jobs/Jobs/org.openoffice.Office.Jobs:Job['UpdateCheck']/Arguments";
        let y = xcu_set(x, job, "AutoCheckEnabled", Some("false")).unwrap();
        assert!(y.contains("Job[&apos;UpdateCheck&apos;]"));
        assert_eq!(xcu_get(&y, job, "AutoCheckEnabled").as_deref(), Some("false"));
        assert_eq!(xcu_set(&y, job, "AutoCheckEnabled", None).unwrap(), x);
        let z = xcu_set(x, "/org.openoffice.Office.Common/Misc", "CrashReport", Some("false")).unwrap();
        assert_eq!(xcu_get(&z, "/org.openoffice.Office.Common/Misc", "CrashReport").as_deref(), Some("false"));
    }

    #[test]
    fn gimprc() {
        let g = "# GIMP gimprc\n(theme \"Dark\")\n\n# end of gimprc\n";
        assert_eq!(gimprc_get(g, "check-updates"), None);
        let h = gimprc_set(g, "check-updates", Some("no"));
        assert!(h.contains("(check-updates no)\n# end of gimprc"));
        assert_eq!(gimprc_get(&h, "check-updates").as_deref(), Some("no"));
        assert_eq!(gimprc_get(&h, "check-update"), None);
        assert_eq!(gimprc_set(&h, "check-updates", None), g);
    }

    #[test]
    fn filezilla() {
        let f = "<FileZilla3>\n\t<Settings>\n\t\t<Setting name=\"Kiosk mode\">0</Setting>\n\t\t<Setting name=\"Master password encryptor\" />\n\t</Settings>\n</FileZilla3>\n";
        assert_eq!(xmlsetting_get(f, "Kiosk mode").as_deref(), Some("0"));
        assert_eq!(xmlsetting_get(f, "Master password encryptor").as_deref(), Some(""));
        let g = xmlsetting_set(f, "Kiosk mode", Some("1")).unwrap();
        assert_eq!(g, f.replace(">0<", ">1<"));
        let h = xmlsetting_set(f, "New", Some("2")).unwrap();
        assert!(h.contains("\t\t<Setting name=\"New\">2</Setting>\n\t</Settings>"));
        assert_eq!(xmlsetting_set(&h, "New", None).unwrap(), f);
    }
}
