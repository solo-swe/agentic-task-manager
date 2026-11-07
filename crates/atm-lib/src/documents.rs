//! A project's files for the document viewer (the Python atm's `lib/documents.py`; docs/compmd-viewer-design.md §4):
//! which ones atm hands out, what each is, and the `CompMD` document the dashboard renders for it. atm renders
//! nothing itself: a code file becomes a fenced block of its language, an image a Markdown image, and `CompMD` does
//! the rest. The rules are here; the reading is the version control's (the project's working copy).

use atm_common::py::splitlines;
use atm_common::time::format_iso;
use atm_common::{Error, Result};
use atm_vcs::GitVcs;
use serde_json::{Value, json};

const DOC_MAX_PATH: usize = 512;
/// A document, code or text.
const DOC_MAX_BYTES: u64 = 1_000_000;
const DOC_MAX_IMAGE_BYTES: u64 = 5_000_000;
/// How much of a file says whether it is text.
const DOC_SNIFF_BYTES: usize = 8192;

const DOC_IMAGES: [(&str, &str); 6] = [
    (".png", "image/png"),
    (".jpg", "image/jpeg"),
    (".jpeg", "image/jpeg"),
    (".gif", "image/gif"),
    (".webp", "image/webp"),
    (".svg", "image/svg+xml"),
];

/// A code file's fence language, by extension (or by whole name, for files without one).
const DOC_LANGUAGES: &[(&str, &str)] = &[
    (".py", "python"),
    (".pyi", "python"),
    (".js", "javascript"),
    (".mjs", "javascript"),
    (".cjs", "javascript"),
    (".jsx", "jsx"),
    (".ts", "ts"),
    (".mts", "ts"),
    (".cts", "ts"),
    (".tsx", "tsx"),
    (".svelte", "svelte"),
    (".vue", "vue"),
    (".json", "json"),
    (".jsonc", "jsonc"),
    (".yaml", "yaml"),
    (".yml", "yaml"),
    (".toml", "toml"),
    (".ini", "ini"),
    (".cfg", "ini"),
    (".xml", "xml"),
    (".html", "html"),
    (".htm", "html"),
    (".css", "css"),
    (".scss", "scss"),
    (".sass", "sass"),
    (".less", "less"),
    (".sh", "sh"),
    (".bash", "bash"),
    (".zsh", "zsh"),
    (".fish", "fish"),
    (".ps1", "powershell"),
    (".bat", "bat"),
    (".rs", "rust"),
    (".go", "go"),
    (".java", "java"),
    (".kt", "kotlin"),
    (".kts", "kotlin"),
    (".scala", "scala"),
    (".groovy", "groovy"),
    (".gradle", "groovy"),
    (".c", "c"),
    (".h", "c"),
    (".cc", "cpp"),
    (".cpp", "cpp"),
    (".cxx", "cpp"),
    (".hpp", "cpp"),
    (".cs", "csharp"),
    (".fs", "fsharp"),
    (".swift", "swift"),
    (".m", "objectivec"),
    (".rb", "ruby"),
    (".php", "php"),
    (".pl", "perl"),
    (".lua", "lua"),
    (".r", "r"),
    (".jl", "julia"),
    (".dart", "dart"),
    (".ex", "elixir"),
    (".exs", "elixir"),
    (".erl", "erlang"),
    (".hs", "haskell"),
    (".clj", "clojure"),
    (".ml", "ocaml"),
    (".zig", "zig"),
    (".nim", "nim"),
    (".sql", "sql"),
    (".graphql", "graphql"),
    (".proto", "protobuf"),
    (".tf", "hcl"),
    (".hcl", "hcl"),
    (".mermaid", "mermaid"),
    (".mmd", "mermaid"),
    (".diff", "diff"),
    (".patch", "diff"),
    ("Dockerfile", "dockerfile"),
    ("Makefile", "makefile"),
];

/// Whether a file name is one that holds keys, refused whatever git says (§4b).
fn key_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let by_extension = ["pem", "key", "p12", "pfx", "keystore", "jks"]
        .iter()
        .any(|e| lower.ends_with(&format!(".{e}")));
    let ssh = ["rsa", "dsa", "ecdsa", "ed25519"].iter().any(|k| {
        let base = format!("id_{k}");
        lower == base || lower.starts_with(&format!("{base}."))
    });
    by_extension || ssh
}

/// The path a request names, checked (§4b): relative to the project's root, `/`-separated, normalized (no `..`, `.`
/// or empty segment), no hidden segment (`.git`, `.env`, `.agentic-task-manager`), no NUL or backslash, at most 512
/// characters.
///
/// # Errors
/// `Invalid` for a path that isn't one; `Refused` for a hidden or key file.
pub fn document_path(path: &str) -> Result<&str> {
    if path.is_empty() || path.chars().count() > DOC_MAX_PATH {
        return Err(Error::Invalid(
            "a file path, relative to the project, of at most 512 characters".into(),
        ));
    }
    if path.contains('\0') || path.contains('\\') || path.starts_with('/') {
        return Err(Error::Invalid(format!(
            "{path}: a relative path with / between its parts"
        )));
    }
    let segments: Vec<&str> = path.split('/').collect();
    if segments.iter().any(|s| matches!(*s, "" | "." | "..")) {
        return Err(Error::Invalid(format!(
            "{path}: a path inside the project, without . or .. parts"
        )));
    }
    if segments.iter().any(|s| s.starts_with('.')) {
        return Err(Error::Refused(format!(
            "{path}: hidden files and folders aren't shown"
        )));
    }
    if segments.last().is_some_and(|name| key_file(name)) {
        return Err(Error::Refused(format!("{path}: key files aren't shown")));
    }
    Ok(path)
}

/// What a file is by its name: `markdown`, `image` (with its content type), `code` (with its fence language), or
/// `text`; `binary` is found only by reading it.
fn document_kind(path: &str) -> (&'static str, Option<&'static str>) {
    let name = path.rsplit('/').next().unwrap_or(path);
    // Python's `splitext`: the last dot's part, not counting leading dots
    let trimmed = name.trim_start_matches('.');
    let extension = trimmed
        .rfind('.')
        .map(|at| trimmed[at..].to_ascii_lowercase())
        .unwrap_or_default();
    if extension == ".md" || extension == ".markdown" {
        return ("markdown", None);
    }
    if let Some((_, kind)) = DOC_IMAGES.iter().find(|(e, _)| *e == extension) {
        return ("image", Some(kind));
    }
    let language = DOC_LANGUAGES
        .iter()
        .find(|(e, _)| *e == extension)
        .or_else(|| DOC_LANGUAGES.iter().find(|(e, _)| *e == name))
        .map(|(_, l)| *l);
    match language {
        Some(language) => ("code", Some(language)),
        None => ("text", None),
    }
}

/// A file the viewer may show: its path, kind, language or content type, and `(size, mtime)`.
struct Checked<'p> {
    path: &'p str,
    kind: &'static str,
    detail: Option<&'static str>,
    stamp: (u64, u128),
}

/// The checks every read of a file passes: its path, a regular file reached through no symlink, not ignored by
/// version control, not too large.
fn checked<'p>(vcs: Option<&GitVcs>, path: &'p str) -> Result<Checked<'p>> {
    let path = document_path(path)?;
    let Some(vcs) = vcs else {
        return Err(Error::NotFound(
            "this project has no working copy to read files from".into(),
        ));
    };
    let Some(stamp) = vcs.file_stamp(path) else {
        return Err(Error::NotFound(format!("{path}: not in the project")));
    };
    if vcs.ignored(path) {
        return Err(Error::Refused(format!(
            "{path}: git ignores it, so it isn't shown"
        )));
    }
    let (kind, detail) = document_kind(path);
    let limit = if kind == "image" {
        DOC_MAX_IMAGE_BYTES
    } else {
        DOC_MAX_BYTES
    };
    if stamp.0 > limit {
        return Err(Error::TooLarge(format!(
            "{path}: larger than {} MB",
            limit / 1_000_000
        )));
    }
    Ok(Checked {
        path,
        kind,
        detail,
        stamp,
    })
}

/// `(size, nanosecond mtime)` of a file the viewer may show, after the same checks as [`read_document`]: for the
/// REST API's `ETag`, so an unchanged file is answered without reading it.
///
/// # Errors
/// As [`read_document`].
pub fn document_stamp(vcs: Option<&GitVcs>, path: &str) -> Result<(u64, u128)> {
    checked(vcs, path).map(|c| c.stamp)
}

/// `text` as a fenced code block of `language`, the fence longer than any run of backticks in it.
fn fence(text: &str, language: Option<&str>) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let marks = "`".repeat((longest + 1).max(3));
    let body = if text.is_empty() || text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{text}\n")
    };
    format!("{marks}{}\n{body}{marks}\n", language.unwrap_or_default())
}

/// Whether the content type's image starts as `data` does (an SVG has `<svg` near its start).
fn image_matches(data: &[u8], content_type: &str) -> bool {
    match content_type {
        "image/svg+xml" => data[..data.len().min(DOC_SNIFF_BYTES)]
            .to_ascii_lowercase()
            .windows(4)
            .any(|w| w == b"<svg"),
        "image/webp" => data.get(..4) == Some(b"RIFF") && data.get(8..12) == Some(b"WEBP"),
        "image/png" => data.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => data.starts_with(b"\xff\xd8\xff"),
        "image/gif" => data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a"),
        _ => false,
    }
}

/// A file name as a Markdown link destination, in angle brackets when it has spaces or parentheses.
fn link_target(name: &str) -> String {
    if name
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '(' | ')' | '<' | '>'))
    {
        format!("<{name}>")
    } else {
        name.to_string()
    }
}

/// A file of the project for the viewer (§4a): `{path, kind, language, text, document, line_offset, lines, bytes,
/// modified_at, tracked, commit, submodule}`. `document` is the `CompMD` document to render: a Markdown file as it is,
/// code and text in a fence (`line_offset` 1), an image as a Markdown image of its own name, a binary file as a line
/// saying so.
///
/// # Errors
/// `Invalid`, `NotFound`, `Refused` or `TooLarge`, by the checks above.
pub fn read_document(vcs: Option<&GitVcs>, path: &str) -> Result<Value> {
    let found = checked(vcs, path)?;
    let vcs = vcs.ok_or_else(|| Error::NotFound("no working copy".into()))?;
    let path = found.path;
    let data = vcs.read_file(path)?;
    let name = path.rsplit('/').next().unwrap_or(path);
    let (mut kind, mut detail) = (found.kind, found.detail);
    let mut text = None;
    let mut line_offset = 0;
    let document = if kind == "image" {
        if image_matches(&data, detail.unwrap_or_default()) {
            format!("![{name}]({})\n", link_target(name))
        } else {
            kind = "binary";
            format!(
                "*{name} isn't the image its name says: {} bytes.*\n",
                data.len()
            )
        }
    } else {
        let binary = data[..data.len().min(DOC_SNIFF_BYTES)].contains(&0);
        match std::str::from_utf8(&data) {
            Ok(decoded) if !binary => {
                text = Some(decoded.to_string());
                if kind == "markdown" {
                    decoded.to_string()
                } else {
                    line_offset = 1;
                    fence(decoded, detail)
                }
            }
            _ => {
                kind = "binary";
                detail = None;
                format!("*{name} is a binary file: {} bytes.*\n", data.len())
            }
        }
    };
    let lines = text.as_deref().map_or(0, |t| splitlines(t).len());
    #[allow(clippy::cast_possible_truncation)]
    // reason: nanoseconds since 1970 in seconds fit an i64
    let modified = format_iso((found.stamp.1 / 1_000_000_000) as i64);
    Ok(json!({
        "path": path,
        "kind": kind,
        "language": if kind == "code" { detail } else { None },
        "text": text,
        "document": document,
        "line_offset": line_offset,
        "lines": lines,
        "bytes": data.len(),
        "modified_at": modified,
        "tracked": vcs.tracked(path),
        "commit": vcs.head(),
        // A file in a submodule lives in the submodule's repository: its folder, commit and web address (T-408)
        "submodule": vcs.submodule_of(path),
    }))
}

/// (bytes, content type) of an image file of the project, for an `<img>` (the files route): only the image types the
/// viewer shows, whose bytes match their name.
///
/// # Errors
/// `Refused` for any other file, as well as [`read_document`]'s errors.
pub fn read_image(vcs: Option<&GitVcs>, path: &str) -> Result<(Vec<u8>, &'static str)> {
    let found = checked(vcs, path)?;
    let ("image", Some(content_type)) = (found.kind, found.detail) else {
        return Err(Error::Refused(format!(
            "{}: only images are served as files",
            found.path
        )));
    };
    let vcs = vcs.ok_or_else(|| Error::NotFound("no working copy".into()))?;
    let data = vcs.read_file(found.path)?;
    if !image_matches(&data, content_type) {
        return Err(Error::Refused(format!(
            "{}: isn't the image its name says",
            found.path
        )));
    }
    Ok((data, content_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_checked_before_anything_is_read() {
        assert!(document_path("docs/x.md").is_ok());
        for bad in ["", "/etc/passwd", "a/../b", "a//b", "a\\b"] {
            assert!(
                matches!(document_path(bad), Err(Error::Invalid(_))),
                "{bad}"
            );
        }
        for refused in [
            ".env",
            "a/.git/config",
            "keys/server.PEM",
            "id_rsa",
            "id_ed25519.pub",
        ] {
            assert!(
                matches!(document_path(refused), Err(Error::Refused(_))),
                "{refused}"
            );
        }
        assert!(document_path("id_rsa_notes.md").is_ok());
    }

    #[test]
    fn a_file_is_known_by_its_name() {
        assert_eq!(document_kind("docs/a.comp.md"), ("markdown", None));
        assert_eq!(document_kind("x.SVG"), ("image", Some("image/svg+xml")));
        assert_eq!(document_kind("src/main.rs"), ("code", Some("rust")));
        assert_eq!(document_kind("Dockerfile"), ("code", Some("dockerfile")));
        assert_eq!(document_kind("notes.txt"), ("text", None));
    }

    #[test]
    fn code_is_fenced_longer_than_its_backticks() {
        assert_eq!(fence("a ```` b", Some("md")), "`````md\na ```` b\n`````\n");
        assert_eq!(fence("x\n", None), "```\nx\n```\n");
        assert_eq!(link_target("a b.png"), "<a b.png>");
        assert!(image_matches(b"GIF89a...", "image/gif"));
        assert!(!image_matches(b"<html>", "image/png"));
    }
}
