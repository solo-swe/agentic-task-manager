//! The rules of components (the Python atm's `common/components.py`; docs/sprints-and-releases-design.md §2, SR-2.7):
//! the parts of a project (`meta.components`, each with globs over its files), the components a task's files suggest,
//! and the IDs a task may name. Listing, defining and changing them is atm-lib's.

use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::ids::valid_short_id;
use crate::py::{py_repr, py_str};
use crate::vocabulary::Vocabulary;

/// One piece of a glob, as Python's `fnmatch` reads it.
#[derive(Debug)]
enum Piece {
    /// `*`: any run of characters, `/` included.
    Star,
    /// `?`: any one character.
    One,
    /// `[…]` (`[!…]` negated): one character of the set, its ranges included.
    Class {
        negated: bool,
        items: Vec<(char, char)>,
    },
    /// Any other character, as itself.
    Literal(char),
}

/// The pieces of a glob. A `[` without its `]` is a plain `[`, as in Python.
fn pieces(glob: &str) -> Vec<Piece> {
    let chars: Vec<char> = glob.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        match c {
            '*' => {
                if !matches!(out.last(), Some(Piece::Star)) {
                    out.push(Piece::Star);
                }
            }
            '?' => out.push(Piece::One),
            '[' => {
                let mut j = i;
                if j < chars.len() && chars[j] == '!' {
                    j += 1;
                }
                if j < chars.len() && chars[j] == ']' {
                    j += 1;
                }
                while j < chars.len() && chars[j] != ']' {
                    j += 1;
                }
                if j >= chars.len() {
                    out.push(Piece::Literal('['));
                    continue;
                }
                let mut set = &chars[i..j];
                i = j + 1;
                let negated = set.first() == Some(&'!');
                if negated {
                    set = &set[1..];
                }
                let mut items = Vec::new();
                let mut k = 0;
                while k < set.len() {
                    if k + 2 < set.len() && set[k + 1] == '-' {
                        // A range; one backwards matches nothing (Python 3.9+)
                        if set[k] <= set[k + 2] {
                            items.push((set[k], set[k + 2]));
                        }
                        k += 3;
                    } else {
                        items.push((set[k], set[k]));
                        k += 1;
                    }
                }
                out.push(Piece::Class { negated, items });
            }
            other => out.push(Piece::Literal(other)),
        }
    }
    out
}

fn matches_one(piece: &Piece, c: char) -> bool {
    match piece {
        Piece::Star | Piece::One => true,
        Piece::Class { negated, items } => {
            items.iter().any(|(lo, hi)| (*lo..=*hi).contains(&c)) != *negated
        }
        Piece::Literal(l) => *l == c,
    }
}

/// Python's `fnmatch.fnmatchcase`: the whole of `text` matches `glob`, case-sensitively. `*` matches across folders.
#[must_use]
pub fn fnmatchcase(text: &str, glob: &str) -> bool {
    let pieces = pieces(glob);
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    // The last star seen, and where in the text it started matching: the place to try again from
    let mut retry: Option<(usize, usize)> = None;
    while t < text.len() {
        match pieces.get(p) {
            Some(Piece::Star) => {
                retry = Some((p, t));
                p += 1;
            }
            Some(piece) if matches_one(piece, text[t]) => {
                p += 1;
                t += 1;
            }
            _ => match retry {
                Some((star, from)) => {
                    p = star + 1;
                    t = from + 1;
                    retry = Some((star, from + 1));
                }
                None => return false,
            },
        }
    }
    pieces[p..].iter().all(|piece| matches!(piece, Piece::Star))
}

/// Whether `path` (a task's file, relative to the project) is in the component: one of its globs matches. `*` and
/// `**` both match across folders (`dashboard/**` is everything under dashboard/).
#[must_use]
pub fn matches(component: &Map<String, Value>, path: &str) -> bool {
    component
        .get("paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(|glob| fnmatchcase(path, glob))
}

fn component_id(component: &Map<String, Value>) -> &str {
    component
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// The components the files are in, in the project's order.
#[must_use]
pub fn suggest_components(vocab: &Vocabulary, files: &[Value]) -> Vec<String> {
    vocab
        .components
        .iter()
        .filter(|c| {
            files
                .iter()
                .filter_map(Value::as_str)
                .any(|f| matches(c, f))
        })
        .map(|c| component_id(c).to_string())
        .collect()
}

/// The component IDs a task may have: in a project with components, only theirs; otherwise any ID of letters, digits,
/// `.`, `-` and `_` (agents put them in shell commands).
///
/// # Errors
/// The first bad one (exit code 2), with the components there are.
pub fn component_ids(vocab: &Vocabulary, values: &[Value]) -> Result<Vec<Value>> {
    let known: Vec<&str> = vocab.components.iter().map(component_id).collect();
    for value in values {
        if !value.as_str().is_some_and(valid_short_id) {
            return Err(Error::Invalid(format!(
                "component {}: use letters, digits, '.', '-' and '_' (at most 64)",
                py_repr(value)
            )));
        }
        if !known.is_empty() && !known.contains(&py_str(value).as_str()) {
            return Err(Error::Invalid(format!(
                "no component {}; the components: {}",
                py_repr(value),
                known.join(", ")
            )));
        }
    }
    Ok(values.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn globs_match_as_fnmatchcase_does() {
        assert!(fnmatchcase("dashboard/src/a.ts", "dashboard/**"));
        assert!(fnmatchcase("dashboard/src/a.ts", "dashboard/*"));
        assert!(fnmatchcase("a/b/c.rs", "*.rs"));
        assert!(!fnmatchcase("a/b/c.rs", "*.ts"));
        assert!(fnmatchcase("x1", "x?"));
        assert!(!fnmatchcase("x", "x?"));
        assert!(fnmatchcase("b", "[abc]"));
        assert!(!fnmatchcase("d", "[abc]"));
        assert!(fnmatchcase("d", "[!abc]"));
        assert!(fnmatchcase("m", "[a-z]"));
        assert!(!fnmatchcase("m", "[z-a]"));
        assert!(fnmatchcase("]", "[]]"));
        assert!(fnmatchcase("[x", "[x"));
        assert!(!fnmatchcase("A", "a"));
        assert!(fnmatchcase("", "*"));
        assert!(fnmatchcase("abcabd", "*abd"));
    }

    fn vocab(meta: &Value) -> Vocabulary {
        Vocabulary::of_meta(meta.as_object().unwrap()).unwrap()
    }

    #[test]
    fn files_suggest_their_components_in_the_projects_order() {
        let v = vocab(&json!({"components": [
            {"id": "cli", "paths": ["atm-cli/**"]},
            {"id": "ui", "paths": ["dashboard/**"]}
        ]}));
        let files = [json!("dashboard/a.ts"), json!(3), json!("atm-cli/x.py")];
        assert_eq!(suggest_components(&v, &files), ["cli", "ui"]);
    }

    #[test]
    fn component_ids_are_checked_against_the_projects() {
        let none = vocab(&json!({}));
        assert!(component_ids(&none, &[json!("any.id")]).is_ok());
        assert!(component_ids(&none, &[json!("a b")]).is_err());
        let v = vocab(&json!({"components": [{"id": "cli"}]}));
        let err = component_ids(&v, &[json!("ui")]).unwrap_err();
        assert_eq!(err.to_string(), "no component 'ui'; the components: cli");
    }
}
