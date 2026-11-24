//! A project's documents for the Documents page (ATM-764, docs/documents-design.md §2–§3): its Markdown files where
//! git has them (tracked, or untracked and not ignored), those the viewer refuses left out, each submodule's by its
//! own repository's rules; each with its title, size, time and whether git tracks it. At most [`LIST_MAX`], the first
//! by path; a query matches titles and paths before that cut, so every document can be found.

use atm_common::time::format_iso;
use atm_common::{Error, Result};
use atm_vcs::GitVcs;
use serde_json::{Value, json};

use crate::{document_kind, document_path};

/// The most documents a listing gives (Hakan, CR-137); past that, how many more.
pub const LIST_MAX: usize = 500;
/// The longest query, in characters.
const QUERY_MAX: usize = 200;
/// How much of a document its title is looked for in.
const TITLE_BYTES: usize = 8192;
/// The longest title, in characters.
const TITLE_MAX: usize = 200;
/// How deep submodules inside submodules are followed: past it, a submodule is listed without its documents.
const SUBMODULE_DEPTH: usize = 8;

/// A document found: its path from the project's root, the repository it is in and its path there, whether that
/// repository tracks it, and its `(size, nanosecond mtime)`.
struct Found {
    path: String,
    vcs: GitVcs,
    inside: String,
    tracked: bool,
    stamp: (u64, u128),
}

/// What a walk of the project's repositories found: its documents (by path), its submodules (nested ones too), and
/// each repository's `HEAD`, the project's first.
struct Walk {
    documents: Vec<Found>,
    submodules: Vec<Value>,
    heads: Vec<String>,
}

/// The documents of the repository `vcs` (shown under `prefix`, the submodule's folder from the project's root), and
/// its submodules', down to [`SUBMODULE_DEPTH`].
fn walk(vcs: &GitVcs, prefix: &str, depth: usize, into: &mut Walk) {
    let Some(listing) = vcs.listed_files() else {
        return;
    };
    into.heads.push(vcs.head().unwrap_or_default());
    for file in listing.files {
        let path = format!("{prefix}{}", file.path);
        // Markdown only (§2), and nothing the viewer refuses: git's and atm's folders, environment and key files
        if document_kind(&path).0 != "markdown" || document_path(&path).is_err() {
            continue;
        }
        // A regular file reached through no symbolic link, as the viewer reads it
        let Some(stamp) = vcs.file_stamp(&file.path) else {
            continue;
        };
        into.documents.push(Found {
            path,
            vcs: vcs.clone(),
            inside: file.path,
            tracked: file.tracked,
            stamp,
        });
    }
    for sub in listing.submodules {
        let path = format!("{prefix}{}", sub.path);
        into.submodules
            .push(json!({"path": path, "checked_out": sub.checked_out}));
        if sub.checked_out
            && depth < SUBMODULE_DEPTH
            && let Some(inner) = vcs.submodule_at(&sub.path)
        {
            walk(&inner, &format!("{path}/"), depth + 1, into);
        }
    }
}

/// The project's documents and submodules, by path; none without git.
fn found(vcs: Option<&GitVcs>) -> Option<Walk> {
    let vcs = vcs.filter(|v| v.in_repository())?;
    let mut into = Walk {
        documents: Vec::new(),
        submodules: Vec::new(),
        heads: Vec::new(),
    };
    walk(vcs, "", 0, &mut into);
    into.documents.sort_by(|a, b| a.path.cmp(&b.path));
    into.documents.dedup_by(|a, b| a.path == b.path);
    into.submodules
        .sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    Some(into)
}

/// A document's title (§3): its first level-one heading in its first 8 KB, an ATX one (`# Title`) or a Setext one
/// (`Title` over `===`), outside front matter and fenced code; else its file's name without `.md` or `.markdown`.
#[must_use]
pub fn document_title(start: &[u8], path: &str) -> String {
    let text = String::from_utf8_lossy(start);
    let mut lines = text.lines().peekable();
    // Front matter: `---` on the first line, to the next `---` or `...`
    if lines.peek().is_some_and(|l| l.trim_end() == "---") {
        lines.next();
        for line in lines.by_ref() {
            if matches!(line.trim_end(), "---" | "...") {
                break;
            }
        }
    }
    let mut fence: Option<&str> = None;
    let mut previous: Option<&str> = None;
    for line in lines {
        let indent = line.len() - line.trim_start_matches(' ').len();
        let body = if indent <= 3 { &line[indent..] } else { "" };
        if let Some(open) = fence {
            if body.trim_end().starts_with(open) {
                fence = None;
            }
            previous = None;
            continue;
        }
        if body.starts_with("```") || body.starts_with("~~~") {
            fence = Some(&body[..3]);
            previous = None;
            continue;
        }
        if let Some(heading) = atx_title(body) {
            return capped(heading);
        }
        let underline = body.trim_end();
        if let Some(above) = previous
            && !underline.is_empty()
            && underline.chars().all(|c| c == '=')
        {
            return capped(above.trim());
        }
        previous = (!body.trim().is_empty() && indent <= 3).then_some(line);
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    let lower = name.to_ascii_lowercase();
    let cut = [".markdown", ".md"]
        .iter()
        .find(|e| lower.ends_with(*e))
        .map_or(name.len(), |e| name.len() - e.len());
    capped(&name[..cut])
}

/// A level-one ATX heading's text (`# Title`, `# Title #`), or none for any other line.
fn atx_title(body: &str) -> Option<&str> {
    let rest = body.strip_prefix('#')?;
    if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return None;
    }
    let text = rest.trim();
    // A closing sequence of #s after a space is not the title's
    let closed = text.trim_end_matches('#');
    let text = if closed.len() < text.len() && (closed.is_empty() || closed.ends_with([' ', '\t']))
    {
        closed.trim_end()
    } else {
        text
    };
    (!text.is_empty()).then_some(text)
}

/// `text` at most [`TITLE_MAX`] characters, cut with `…`.
fn capped(text: &str) -> String {
    if text.chars().count() <= TITLE_MAX {
        return text.to_string();
    }
    text.chars().take(TITLE_MAX - 1).collect::<String>() + "…"
}

/// Whether every word of `query` (lowercased) is in the document's title or path.
fn matches(words: &[String], title: &str, path: &str) -> bool {
    let (title, path) = (title.to_lowercase(), path.to_lowercase());
    words
        .iter()
        .all(|w| title.contains(w.as_str()) || path.contains(w.as_str()))
}

/// The project's documents (docs/documents-design.md §2–§3, §5): `{git, documents: [{path, title, bytes,
/// modified_at, tracked}], submodules: [{path, checked_out}], more}`, the documents by path, at most [`LIST_MAX`], and
/// `more` those past it. `query` (every word in a document's title or path, any case) is matched before the cut.
/// Without git: no documents (`git` false), the page says why.
///
/// # Errors
/// `Invalid` for a query of more than 200 characters.
pub fn list_documents(vcs: Option<&GitVcs>, query: &str) -> Result<Value> {
    let query = query.trim();
    if query.chars().count() > QUERY_MAX {
        return Err(Error::Invalid(format!(
            "a search of at most {QUERY_MAX} characters"
        )));
    }
    let Some(walk) = found(vcs) else {
        return Ok(json!({"git": false, "documents": [], "submodules": [], "more": 0}));
    };
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let title = |f: &Found| {
        let start = f
            .vcs
            .read_file_start(&f.inside, TITLE_BYTES)
            .unwrap_or_default();
        document_title(&start, &f.path)
    };
    // Without a query only the documents listed are read for their titles; with one, every document is
    let mut shown: Vec<(Found, String)> = Vec::new();
    let mut total = 0;
    for document in walk.documents {
        if words.is_empty() {
            total += 1;
            if shown.len() < LIST_MAX {
                let t = title(&document);
                shown.push((document, t));
            }
            continue;
        }
        let t = title(&document);
        if matches(&words, &t, &document.path) {
            total += 1;
            if shown.len() < LIST_MAX {
                shown.push((document, t));
            }
        }
    }
    let documents: Vec<Value> = shown
        .iter()
        .map(|(f, title)| {
            json!({
                "path": f.path,
                "title": title,
                "bytes": f.stamp.0,
                "modified_at": format_iso(i64::try_from(f.stamp.1 / 1_000_000_000).unwrap_or(i64::MAX)),
                "tracked": f.tracked,
            })
        })
        .collect();
    Ok(json!({
        "git": true,
        "more": total - documents.len(),
        "documents": documents,
        "submodules": walk.submodules,
    }))
}

/// What a listing is made from, cheap to read, for its `ETag` (§3): each repository's `HEAD`, and every document's
/// path, size and time, the submodules' and the untracked ones too. The same text while none of them changes; none
/// without git.
#[must_use]
pub fn documents_stamp(vcs: Option<&GitVcs>) -> Option<String> {
    let walk = found(vcs)?;
    let mut lines = vec![walk.heads.join(" ")];
    lines.extend(
        walk.documents
            .iter()
            .map(|f| format!("{}\t{}\t{}\t{}", f.path, f.stamp.0, f.stamp.1, f.tracked)),
    );
    lines.extend(walk.submodules.iter().map(Value::to_string));
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests;
