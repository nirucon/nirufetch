//! A small hand-rolled JSON writer for `--json` (no serde dependency).

pub enum Json {
    Null,
    Int(u64),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(&'static str, Json)>),
}

impl Json {
    pub fn str(value: impl Into<String>) -> Self {
        Json::Str(value.into())
    }

    pub fn opt_str(value: Option<impl Into<String>>) -> Self {
        value.map(Json::str).unwrap_or(Json::Null)
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Int(value) => out.push_str(&value.to_string()),
            Json::Num(value) if value.is_finite() => out.push_str(&value.to_string()),
            Json::Num(_) => out.push_str("null"),
            Json::Str(value) => escape(value, out),
            Json::Arr(items) if items.is_empty() => out.push_str("[]"),
            Json::Arr(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    newline(out, depth + 1);
                    item.write(out, depth + 1);
                }
                newline(out, depth);
                out.push(']');
            }
            Json::Obj(fields) if fields.is_empty() => out.push_str("{}"),
            Json::Obj(fields) => {
                out.push('{');
                for (index, (key, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    newline(out, depth + 1);
                    escape(key, out);
                    out.push_str(": ");
                    value.write(out, depth + 1);
                }
                newline(out, depth);
                out.push('}');
            }
        }
    }
}

fn newline(out: &mut String, depth: usize) {
    out.push('\n');
    for _ in 0..depth {
        out.push_str("  ");
    }
}

/// Quote a string: `"` and `\` are escaped, control characters become
/// `\n`/`\t`/… or `\u00XX`; everything else is written as UTF-8.
pub fn escape(value: &str, out: &mut String) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || c == '\u{7f}' || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A strict JSON checker for the tests: returns the parsed value's shape so
/// tests can look inside, or an error with the byte offset.
#[cfg(test)]
pub mod check {
    #[derive(Debug, PartialEq)]
    pub enum Value {
        Null,
        Bool(bool),
        Num(f64),
        Str(String),
        Arr(Vec<Value>),
        Obj(Vec<(String, Value)>),
    }

    impl Value {
        pub fn get(&self, key: &str) -> Option<&Value> {
            match self {
                Value::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }
    }

    pub fn parse(text: &str) -> Result<Value, String> {
        let bytes = text.as_bytes();
        let mut at = 0;
        let value = value(bytes, &mut at)?;
        skip_ws(bytes, &mut at);
        if at != bytes.len() {
            return Err(format!("trailing data at {at}"));
        }
        Ok(value)
    }

    fn skip_ws(b: &[u8], at: &mut usize) {
        while *at < b.len() && matches!(b[*at], b' ' | b'\n' | b'\r' | b'\t') {
            *at += 1;
        }
    }

    fn expect(b: &[u8], at: &mut usize, lit: &str) -> Result<(), String> {
        if b[*at..].starts_with(lit.as_bytes()) {
            *at += lit.len();
            Ok(())
        } else {
            Err(format!("expected {lit} at {at}"))
        }
    }

    fn value(b: &[u8], at: &mut usize) -> Result<Value, String> {
        skip_ws(b, at);
        match b.get(*at) {
            Some(b'n') => expect(b, at, "null").map(|_| Value::Null),
            Some(b't') => expect(b, at, "true").map(|_| Value::Bool(true)),
            Some(b'f') => expect(b, at, "false").map(|_| Value::Bool(false)),
            Some(b'"') => string(b, at).map(Value::Str),
            Some(b'[') => {
                *at += 1;
                let mut items = Vec::new();
                skip_ws(b, at);
                if b.get(*at) == Some(&b']') {
                    *at += 1;
                    return Ok(Value::Arr(items));
                }
                loop {
                    items.push(value(b, at)?);
                    skip_ws(b, at);
                    match b.get(*at) {
                        Some(b',') => *at += 1,
                        Some(b']') => {
                            *at += 1;
                            return Ok(Value::Arr(items));
                        }
                        _ => return Err(format!("expected , or ] at {at}")),
                    }
                }
            }
            Some(b'{') => {
                *at += 1;
                let mut fields = Vec::new();
                skip_ws(b, at);
                if b.get(*at) == Some(&b'}') {
                    *at += 1;
                    return Ok(Value::Obj(fields));
                }
                loop {
                    skip_ws(b, at);
                    if b.get(*at) != Some(&b'"') {
                        return Err(format!("expected key at {at}"));
                    }
                    let key = string(b, at)?;
                    if fields.iter().any(|(k, _): &(String, Value)| *k == key) {
                        return Err(format!("duplicate key {key}"));
                    }
                    skip_ws(b, at);
                    expect(b, at, ":")?;
                    fields.push((key, value(b, at)?));
                    skip_ws(b, at);
                    match b.get(*at) {
                        Some(b',') => *at += 1,
                        Some(b'}') => {
                            *at += 1;
                            return Ok(Value::Obj(fields));
                        }
                        _ => return Err(format!("expected , or }} at {at}")),
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => number(b, at),
            _ => Err(format!("unexpected byte at {at}")),
        }
    }

    fn number(b: &[u8], at: &mut usize) -> Result<Value, String> {
        let start = *at;
        if b.get(*at) == Some(&b'-') {
            *at += 1;
        }
        let digits = |at: &mut usize| {
            let from = *at;
            while *at < b.len() && b[*at].is_ascii_digit() {
                *at += 1;
            }
            *at - from
        };
        let first = b.get(*at).copied();
        let int = digits(at);
        if int == 0 || (first == Some(b'0') && int > 1) {
            return Err(format!("bad number at {start}"));
        }
        if b.get(*at) == Some(&b'.') {
            *at += 1;
            if digits(at) == 0 {
                return Err(format!("bad fraction at {start}"));
            }
        }
        if matches!(b.get(*at), Some(b'e' | b'E')) {
            *at += 1;
            if matches!(b.get(*at), Some(b'+' | b'-')) {
                *at += 1;
            }
            if digits(at) == 0 {
                return Err(format!("bad exponent at {start}"));
            }
        }
        let text = std::str::from_utf8(&b[start..*at]).unwrap();
        text.parse()
            .map(Value::Num)
            .map_err(|_| format!("bad number at {start}"))
    }

    fn string(b: &[u8], at: &mut usize) -> Result<String, String> {
        *at += 1; // opening quote
        let mut out = String::new();
        loop {
            let Some(&byte) = b.get(*at) else {
                return Err("unterminated string".into());
            };
            match byte {
                b'"' => {
                    *at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    let Some(&esc) = b.get(*at + 1) else {
                        return Err("bad escape".into());
                    };
                    *at += 2;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = b.get(*at..*at + 4).ok_or("short \\u escape")?;
                            let hex = std::str::from_utf8(hex).map_err(|e| e.to_string())?;
                            let code =
                                u32::from_str_radix(hex, 16).map_err(|_| "bad \\u escape")?;
                            *at += 4;
                            out.push(char::from_u32(code).ok_or("surrogate in \\u escape")?);
                        }
                        _ => return Err(format!("bad escape at {at}")),
                    }
                }
                0x00..=0x1f => return Err(format!("raw control character at {at}")),
                _ => {
                    let rest = std::str::from_utf8(&b[*at..]).map_err(|e| e.to_string())?;
                    let c = rest.chars().next().unwrap();
                    out.push(c);
                    *at += c.len_utf8();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::check::{parse, Value};
    use super::*;

    #[test]
    fn strings_are_escaped() {
        let tricky =
            "quote \" back \\ nl \n tab \t cr \r bell \u{07} del \u{7f} ls \u{2028} ä 漢 🦀";
        let mut out = String::new();
        escape(tricky, &mut out);
        assert_eq!(
            out,
            "\"quote \\\" back \\\\ nl \\n tab \\t cr \\r bell \\u0007 del \\u007f ls \\u2028 ä 漢 🦀\""
        );
        assert_eq!(parse(&out), Ok(Value::Str(tricky.to_string())));
    }

    #[test]
    fn documents_are_valid_json() {
        let doc = Json::Obj(vec![
            ("a", Json::Null),
            ("b", Json::Int(18_446_744_073_709_551_615)),
            ("c", Json::Num(0.52)),
            ("d", Json::Num(f64::NAN)),
            ("e", Json::Arr(vec![])),
            ("f", Json::Obj(vec![])),
            (
                "g",
                Json::Arr(vec![
                    Json::str("x\"y"),
                    Json::Obj(vec![("h", Json::Int(1))]),
                ]),
            ),
        ]);
        let value = parse(&doc.render()).expect("valid JSON");
        assert_eq!(value.get("c"), Some(&Value::Num(0.52)));
        assert_eq!(value.get("d"), Some(&Value::Null));
        assert!(parse("{\"a\": 01}").is_err());
        assert!(parse("{\"a\": \"\n\"}").is_err());
        assert!(parse("{\"a\": 1,}").is_err());
        assert!(parse("[1] x").is_err());
    }
}
