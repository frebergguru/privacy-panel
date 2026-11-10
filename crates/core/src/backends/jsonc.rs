//! JSON and JSON-with-comments files (VS Code settings, browser preferences, policy files).
//! The text is parsed into spans and only the target value's span is replaced, so comments,
//! formatting and key order stay exactly as they were.

use std::ops::Range;

#[derive(Debug)]
enum Node {
    Object { span: Range<usize>, members: Vec<Member> },
    Other { span: Range<usize> },
}

impl Node {
    fn span(&self) -> Range<usize> {
        match self {
            Node::Object { span, .. } | Node::Other { span } => span.clone(),
        }
    }
}

#[derive(Debug)]
struct Member {
    key: String,
    /// Start of the key's opening quote.
    key_start: usize,
    value: Node,
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

type PResult<T> = Result<T, String>;

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        loop {
            while self.i < self.s.len() && (self.s[self.i] as char).is_ascii_whitespace() {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"//") {
                while self.i < self.s.len() && self.s[self.i] != b'\n' {
                    self.i += 1;
                }
            } else if self.s[self.i..].starts_with(b"/*") {
                match find(&self.s[self.i + 2..], b"*/") {
                    Some(p) => self.i += p + 4,
                    None => self.i = self.s.len(),
                }
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn string(&mut self) -> PResult<String> {
        let start = self.i;
        self.i += 1; // opening quote
        while self.i < self.s.len() {
            match self.s[self.i] {
                b'\\' => self.i += 2,
                b'"' => {
                    self.i += 1;
                    let raw = std::str::from_utf8(&self.s[start..self.i]).map_err(|e| e.to_string())?;
                    return serde_json::from_str::<String>(raw).map_err(|e| e.to_string());
                }
                _ => self.i += 1,
            }
        }
        Err("unterminated string".into())
    }

    fn value(&mut self) -> PResult<Node> {
        self.skip_ws();
        let start = self.i;
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => {
                self.i += 1;
                loop {
                    self.skip_ws();
                    match self.peek() {
                        Some(b']') => {
                            self.i += 1;
                            break;
                        }
                        Some(b',') => self.i += 1,
                        None => return Err("unterminated array".into()),
                        _ => {
                            self.value()?;
                        }
                    }
                }
                Ok(Node::Other { span: start..self.i })
            }
            Some(b'"') => {
                self.string()?;
                Ok(Node::Other { span: start..self.i })
            }
            Some(_) => {
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'+' | b'.') {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                if self.i == start {
                    return Err(format!("unexpected character at byte {start}"));
                }
                Ok(Node::Other { span: start..self.i })
            }
            None => Err("unexpected end of file".into()),
        }
    }

    fn object(&mut self) -> PResult<Node> {
        let start = self.i;
        self.i += 1;
        let mut members = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Node::Object { span: start..self.i, members });
                }
                Some(b',') => self.i += 1,
                Some(b'"') => {
                    let key_start = self.i;
                    let key = self.string()?;
                    self.skip_ws();
                    if self.peek() != Some(b':') {
                        return Err(format!("expected ':' at byte {}", self.i));
                    }
                    self.i += 1;
                    let value = self.value()?;
                    members.push(Member { key, key_start, value });
                }
                _ => return Err(format!("unexpected character in object at byte {}", self.i)),
            }
        }
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn parse(text: &str) -> PResult<Node> {
    let mut p = Parser { s: text.as_bytes(), i: 0 };
    let n = p.value()?;
    if !matches!(n, Node::Object { .. }) {
        return Err("top level is not an object".into());
    }
    Ok(n)
}

/// The raw JSON text of the value at `pointer`, if present.
pub fn get(text: &str, pointer: &[String]) -> Result<Option<String>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let mut node = &parse(text)?;
    for key in pointer {
        match node {
            Node::Object { members, .. } => match members.iter().rev().find(|m| &m.key == key) {
                Some(m) => node = &m.value,
                None => return Ok(None),
            },
            Node::Other { .. } => return Ok(None),
        }
    }
    Ok(Some(text[node.span()].to_string()))
}

fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

fn line_indent(text: &str, pos: usize) -> String {
    let line_start = text[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    text[line_start..pos].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
}

/// Build `"a": {"b": raw}` for the missing tail of a pointer.
fn nested(keys: &[String], raw: &str) -> String {
    let mut v = raw.to_string();
    for k in keys[1..].iter().rev() {
        v = format!("{{{}: {}}}", quote(k), v);
    }
    format!("{}: {}", quote(&keys[0]), v)
}

fn skip_ws(b: &[u8], mut k: usize) -> usize {
    while k < b.len() && (b[k] as char).is_ascii_whitespace() {
        k += 1;
    }
    k
}

/// Remove member `i`: its own line (when it is alone on it) and one comma. Comments
/// on other lines, including ones just before it, are kept.
fn remove_member(text: &str, members: &[Member], i: usize) -> String {
    let b = text.as_bytes();
    let m = &members[i];
    let vend = m.value.span().end;
    let own_comma = {
        let k = skip_ws(b, vend);
        (k < b.len() && b[k] == b',').then_some(k)
    };
    // Start: the beginning of the line if only indentation precedes the key,
    // otherwise just the key with the spaces before it.
    let mut start = m.key_start;
    while start > 0 && matches!(b[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    let line_start = start == 0 || b[start - 1] == b'\n';
    let mut end = match own_comma {
        Some(c) => c + 1,
        None => vend,
    };
    if line_start {
        // Take the rest of the line too if it is only whitespace.
        let mut k = end;
        while k < b.len() && matches!(b[k], b' ' | b'\t' | b'\r') {
            k += 1;
        }
        if k < b.len() && b[k] == b'\n' {
            end = k + 1;
        } else if own_comma.is_some() {
            // Next member on the same line: keep its position.
            end = skip_ws(b, end);
        }
    } else if own_comma.is_some() {
        end = skip_ws(b, end).min(b.len());
        // Keep one space before what follows on the same line.
        if end > 0 && end < b.len() && b[end - 1] == b' ' && start > 0 && b[start - 1] != b' ' {
            end -= 1;
        }
    }
    let mut out = String::with_capacity(text.len());
    if own_comma.is_none() && i > 0 {
        // Last member: drop the comma after the previous member instead.
        let k = skip_ws(b, members[i - 1].value.span().end);
        if k < start && b[k] == b',' {
            out.push_str(&text[..k]);
            out.push_str(&text[k + 1..start]);
        } else {
            out.push_str(&text[..start]);
        }
    } else {
        out.push_str(&text[..start]);
    }
    out.push_str(&text[end..]);
    out
}

/// Set (`Some(raw JSON)`) or remove (`None`) the value at `pointer`.
pub fn set(text: &str, pointer: &[String], raw: Option<&str>) -> Result<String, String> {
    if let Some(r) = raw {
        serde_json::from_str::<serde_json::Value>(r).map_err(|e| format!("invalid value {r}: {e}"))?;
    }
    if pointer.is_empty() {
        return Err("empty pointer".into());
    }
    if text.trim().is_empty() {
        return Ok(match raw {
            None => text.to_string(),
            Some(r) => format!("{{\n    {}\n}}\n", nested(pointer, r)),
        });
    }
    let root = parse(text)?;
    let mut node = &root;
    for (depth, key) in pointer.iter().enumerate() {
        let Node::Object { span, members } = node else {
            return Err(format!("\"{}\" is not an object", pointer[..depth].join("/")));
        };
        let last = depth == pointer.len() - 1;
        let idx = members.iter().rposition(|m| &m.key == key);
        match (idx, raw) {
            (Some(i), _) if !last => node = &members[i].value,
            (Some(i), Some(r)) => {
                let sp = members[i].value.span();
                return Ok(format!("{}{}{}", &text[..sp.start], r, &text[sp.end..]));
            }
            (Some(i), None) => return Ok(remove_member(text, members, i)),
            (None, None) => return Ok(text.to_string()),
            (None, Some(r)) => {
                let entry = nested(&pointer[depth..], r);
                if let Some(lm) = members.last() {
                    let indent = line_indent(text, lm.key_start);
                    let pos = lm.value.span().end;
                    let multiline = text[span.start..lm.key_start].contains('\n');
                    let ins = if multiline {
                        format!(",\n{indent}{entry}")
                    } else {
                        format!(", {entry}")
                    };
                    return Ok(format!("{}{}{}", &text[..pos], ins, &text[pos..]));
                }
                // Empty object: insert right after "{", keeping any comments inside.
                let pos = span.start + 1;
                let close = span.end - 1;
                let base = line_indent(text, span.start);
                let inner = &text[pos..close];
                let ins = if inner.trim().is_empty() {
                    format!("\n{base}    {entry}\n{base}")
                } else {
                    format!("\n{base}    {entry}")
                };
                let rest = if inner.trim().is_empty() { &text[close..] } else { &text[pos..] };
                return Ok(format!("{}{}{}", &text[..pos], ins, rest));
            }
        }
    }
    unreachable!()
}

/// Parse a whole JSONC document into serde_json (comments and trailing commas removed).
pub fn to_value(text: &str) -> Result<serde_json::Value, String> {
    let root = parse(text)?;
    fn conv(text: &str, n: &Node) -> Result<serde_json::Value, String> {
        match n {
            Node::Object { members, .. } => {
                let mut m = serde_json::Map::new();
                for mem in members {
                    m.insert(mem.key.clone(), conv(text, &mem.value)?);
                }
                Ok(serde_json::Value::Object(m))
            }
            Node::Other { span } => {
                let raw = &text[span.clone()];
                serde_json::from_str(raw).or_else(|_| {
                    // Arrays may contain comments or trailing commas; fall back to a string.
                    Ok(serde_json::Value::String(raw.to_string()))
                })
            }
        }
    }
    conv(text, &root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Vec<String> {
        s.split('/').map(String::from).collect()
    }

    const VS: &str = "{\n    // comment\n    \"editor.fontSize\": 14,\n    \"telemetry.telemetryLevel\": \"all\", /* x */\n    \"a\": {\"b\": [1, 2,], \"c\": true},\n}\n";

    #[test]
    fn get_jsonc() {
        assert_eq!(get(VS, &p("telemetry.telemetryLevel")).unwrap().as_deref(), Some("\"all\""));
        assert_eq!(get(VS, &p("a/c")).unwrap().as_deref(), Some("true"));
        assert_eq!(get(VS, &p("a/zz")).unwrap(), None);
        assert_eq!(get(VS, &p("editor.fontSize/x")).unwrap(), None);
    }

    #[test]
    fn replace_keeps_comments() {
        let out = set(VS, &p("telemetry.telemetryLevel"), Some("\"off\"")).unwrap();
        assert_eq!(out, VS.replace("\"all\"", "\"off\""));
        let back = set(&out, &p("telemetry.telemetryLevel"), Some("\"all\"")).unwrap();
        assert_eq!(back, VS);
    }

    #[test]
    fn insert_and_remove_roundtrip() {
        let out = set(VS, &p("new.key"), Some("false")).unwrap();
        assert!(out.contains("\"c\": true},\n    \"new.key\": false,\n}"), "{out}");
        assert_eq!(set(&out, &p("new.key"), None).unwrap(), VS);
    }

    #[test]
    fn insert_nested_compact() {
        let t = r#"{"brave":{"p3a":{"other":1}},"x":2}"#;
        let out = set(t, &p("brave/p3a/enabled"), Some("false")).unwrap();
        assert_eq!(out, r#"{"brave":{"p3a":{"other":1, "enabled": false}},"x":2}"#);
        let out2 = set(t, &p("brave/stats/reporting_enabled"), Some("false")).unwrap();
        assert_eq!(get(&out2, &p("brave/stats/reporting_enabled")).unwrap().as_deref(), Some("false"));
        assert!(to_value(&out2).is_ok());
    }

    #[test]
    fn remove_first_and_only() {
        let t = "{\n  \"a\": 1,\n  \"b\": 2\n}\n";
        assert_eq!(set(t, &p("a"), None).unwrap(), "{\n  \"b\": 2\n}\n");
        assert_eq!(set(t, &p("b"), None).unwrap(), "{\n  \"a\": 1\n}\n");
        let one = "{\"a\": 1}";
        assert_eq!(set(one, &p("a"), None).unwrap(), "{}");
    }

    #[test]
    fn comments_survive_insert_and_remove() {
        let vs = "{\n\t// Place your settings in this file\n}\n";
        let a = set(vs, &p("telemetry.telemetryLevel"), Some("\"off\"")).unwrap();
        assert!(a.contains("// Place your settings"), "{a}");
        assert_eq!(get(&a, &p("telemetry.telemetryLevel")).unwrap().as_deref(), Some("\"off\""));
        let back = set(&a, &p("telemetry.telemetryLevel"), None).unwrap();
        assert!(back.contains("// Place your settings"), "{back}");
        assert!(to_value(&back).is_ok());

        let t = "{\n    \"a\": 1,\n    // \"old.setting\": true,\n    // note\n    \"b\": 2\n}\n";
        let r = set(t, &p("b"), None).unwrap();
        assert_eq!(r, "{\n    \"a\": 1\n    // \"old.setting\": true,\n    // note\n}\n");
        let three = "{\n  \"a\": 1,\n  \"b\": 2,\n  \"c\": 3\n}";
        assert_eq!(set(three, &p("b"), None).unwrap(), "{\n  \"a\": 1,\n  \"c\": 3\n}");
        assert_eq!(set(r#"{"x":1, "y":2, "z":3}"#, &p("y"), None).unwrap(), r#"{"x":1, "z":3}"#);
    }

    #[test]
    fn empty_and_new_files() {
        let out = set("", &p("policies/DisableTelemetry"), Some("true")).unwrap();
        assert_eq!(get(&out, &p("policies/DisableTelemetry")).unwrap().as_deref(), Some("true"));
        let e = set("{}", &p("k"), Some("1")).unwrap();
        assert_eq!(get(&e, &p("k")).unwrap().as_deref(), Some("1"));
        assert!(set("{}", &p("k"), Some("not json")).is_err());
    }
}
