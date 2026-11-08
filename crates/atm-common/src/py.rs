//! Text as the Python atm writes it, for output that stays identical through the port (docs/porting-to-rust.md §1):
//! JSON as `json.dumps(…, indent=2, ensure_ascii=False)` prints it, and values in messages and text as Python's
//! `repr()` and `str()` show them, with its truthiness, `strip()` and `splitlines()`.

use std::fmt::Write;

use serde_json::Value;

/// JSON as atm prints it: two-space indent, `": "` between a key and its value, non-ASCII as it is.
///
/// # Panics
/// Never: a `serde_json::Value` always serializes.
#[must_use]
pub fn to_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).expect("a serde_json Value always serializes")
}

/// JSON on one line as Python's `json.dumps(value, ensure_ascii=False)` writes it, `", "` and `": "` between the
/// parts: what atm measures a task's size by (`project status`'s largest tasks).
///
/// # Panics
/// Never: a `serde_json::Value` always serializes.
#[must_use]
pub fn dumps(value: &Value) -> String {
    match value {
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(dumps).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}: {}", dumps(&Value::String(k.clone())), dumps(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Number(n) if n.is_f64() => n.as_f64().map_or_else(|| n.to_string(), float_repr),
        other => serde_json::to_string(other).expect("a serde_json Value always serializes"),
    }
}

/// Whether Python would take the value as true in `if value:`.
#[must_use]
pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `str(value)`: a string as it is, anything else as `repr`.
#[must_use]
pub fn py_str(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => py_repr(other),
    }
}

/// `str()` of a value that may be missing (`dict.get` gives `None`).
#[must_use]
pub fn py_str_opt(value: Option<&Value>) -> String {
    value.map_or_else(|| "None".to_string(), py_str)
}

/// `repr(value)`, as Python shows JSON's values: `None`, `True`, `'text'`, `[1, 'a']`, `{'k': 2}`.
#[must_use]
pub fn py_repr(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Number(n) => {
            if n.is_f64() {
                n.as_f64().map_or_else(|| n.to_string(), float_repr)
            } else {
                n.to_string()
            }
        }
        Value::String(s) => repr_str(s),
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(py_repr).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}: {}", repr_str(k), py_repr(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `repr()` of a value that may be missing.
#[must_use]
pub fn py_repr_opt(value: Option<&Value>) -> String {
    value.map_or_else(|| "None".to_string(), py_repr)
}

/// `repr(text)`: in single quotes, or double quotes when the text has a single quote and no double one; backslashes,
/// that quote and characters Python doesn't print escaped.
#[must_use]
pub fn repr_str(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(text.len() + 2);
    out.push(quote);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if !printable(c) => {
                let code = u32::from(c);
                // Writing to a String can't fail
                let _ = if code < 0x100 {
                    write!(out, "\\x{code:02x}")
                } else if code < 0x1_0000 {
                    write!(out, "\\u{code:04x}")
                } else {
                    write!(out, "\\U{code:08x}")
                };
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Python's `str.isprintable()` for one character, as far as atm's data needs it: control and format characters and
/// separators other than the space are not.
fn printable(c: char) -> bool {
    if c == ' ' {
        return true;
    }
    !(c.is_control()
        || c.is_whitespace()
        || matches!(c, '\u{ad}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{feff}'))
}

/// `repr(float)`: the shortest digits that read back as the same number, in fixed notation from 1e-4 up to 1e16
/// and scientific otherwise (`1.0`, `0.0001`, `1e+16`, `1.5e-05`).
fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }
    // `{:e}` gives the shortest round-trip digits: "1.5e-5", "1e16"
    let scientific = format!("{f:e}");
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("{:e} always has an exponent");
    let exponent: i32 = exponent.parse().expect("{:e}'s exponent is a number");
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let sign = if negative { "-" } else { "" };
    if (-4..16).contains(&exponent) {
        let point = exponent + 1; // where the decimal point goes in `digits`
        let text = if point <= 0 {
            format!(
                "0.{}{digits}",
                "0".repeat(usize::try_from(-point).unwrap_or(0))
            )
        } else {
            let point = usize::try_from(point).unwrap_or(0);
            if digits.len() <= point {
                format!("{digits}{}.0", "0".repeat(point - digits.len()))
            } else {
                format!("{}.{}", &digits[..point], &digits[point..])
            }
        };
        format!("{sign}{text}")
    } else {
        let rest = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        let exp_sign = if exponent < 0 { '-' } else { '+' };
        format!(
            "{sign}{}{rest}e{exp_sign}{:02}",
            &digits[..1],
            exponent.abs()
        )
    }
}

/// `text.strip()`: Python's whitespace at both ends (Unicode's, and the separators U+001C to U+001F).
#[must_use]
pub fn strip(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
}

/// `text.splitlines()`: lines split at every line boundary Python knows (`\n`, `\r\n`, `\r`, `\v`, `\f`, U+001C to
/// U+001E, U+0085, U+2028, U+2029), without them; no empty last line for a trailing boundary.
#[must_use]
pub fn splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let boundary = matches!(
            c,
            '\n' | '\r'
                | '\u{0b}'
                | '\u{0c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if boundary {
            lines.push(&text[start..i]);
            let mut end = i + c.len_utf8();
            if c == '\r' && chars.peek().is_some_and(|&(_, next)| next == '\n') {
                chars.next();
                end += 1;
            }
            start = end;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// `text.capitalize()`: the first character in upper case, the rest in lower case.
#[must_use]
pub fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first
            .to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_is_printed_as_python_prints_it() {
        assert_eq!(
            to_json(&json!({"a": [1, "é"], "b": {}, "c": []})),
            "{\n  \"a\": [\n    1,\n    \"é\"\n  ],\n  \"b\": {},\n  \"c\": []\n}"
        );
    }

    #[test]
    fn dumps_writes_one_line_as_python_does() {
        assert_eq!(
            dumps(&json!({"id": "T-1", "n": [1, 2.5, null], "é": true})),
            "{\"id\": \"T-1\", \"n\": [1, 2.5, null], \"é\": true}"
        );
        assert_eq!(dumps(&json!("a\"b\n")), "\"a\\\"b\\n\"");
    }

    #[test]
    fn keys_keep_their_order() {
        let value: Value = serde_json::from_str(r#"{"z": 1, "a": 2}"#).unwrap();
        assert_eq!(to_json(&value), "{\n  \"z\": 1,\n  \"a\": 2\n}");
    }

    #[test]
    fn repr_shows_values_as_python_does() {
        assert_eq!(py_repr(&json!(null)), "None");
        assert_eq!(py_repr(&json!(true)), "True");
        assert_eq!(py_repr(&json!("a b")), "'a b'");
        assert_eq!(py_repr(&json!("it's")), "\"it's\"");
        assert_eq!(py_repr(&json!("it's \"x\"")), "'it\\'s \"x\"'");
        assert_eq!(py_repr(&json!("a\nb\\c\u{7}")), "'a\\nb\\\\c\\x07'");
        assert_eq!(py_repr(&json!([1, "a", null])), "[1, 'a', None]");
        assert_eq!(py_repr(&json!({"k": 2})), "{'k': 2}");
        assert_eq!(py_repr(&json!("é\u{a0}\u{2028}")), "'é\\xa0\\u2028'");
    }

    #[test]
    fn floats_read_as_python_reprs_them() {
        for (f, text) in [
            (1.0, "1.0"),
            (1.5, "1.5"),
            (100.0, "100.0"),
            (0.0001, "0.0001"),
            (0.000_015, "1.5e-05"),
            (1e16, "1e+16"),
            (1.25e20, "1.25e+20"),
            (-2.5, "-2.5"),
            (123_456.789, "123456.789"),
        ] {
            assert_eq!(float_repr(f), text, "{f}");
        }
    }

    #[test]
    fn str_is_a_string_as_it_is_and_anything_else_repr() {
        assert_eq!(py_str(&json!("a")), "a");
        assert_eq!(py_str(&json!(3)), "3");
        assert_eq!(py_str_opt(None), "None");
    }

    #[test]
    fn truthiness_as_python() {
        for falsy in [
            json!(null),
            json!(false),
            json!(0),
            json!(0.0),
            json!(""),
            json!([]),
            json!({}),
        ] {
            assert!(!truthy(&falsy), "{falsy}");
        }
        for true_ in [
            json!(true),
            json!(1),
            json!("x"),
            json!([0]),
            json!({"a": null}),
        ] {
            assert!(truthy(&true_), "{true_}");
        }
    }

    #[test]
    fn splitlines_splits_where_python_does() {
        assert_eq!(
            splitlines("a\nb\r\nc\rd\u{2028}e\n"),
            ["a", "b", "c", "d", "e"]
        );
        assert_eq!(splitlines("a\n\nb"), ["a", "", "b"]);
        assert_eq!(splitlines(""), Vec::<&str>::new());
    }

    #[test]
    fn strip_and_capitalize() {
        assert_eq!(strip(" \u{1f}a b\n "), "a b");
        assert_eq!(capitalize("in progress"), "In progress");
        assert_eq!(capitalize("ÉTÉ"), "Été");
        assert_eq!(capitalize(""), "");
    }
}
