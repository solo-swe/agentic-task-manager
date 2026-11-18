//! A project's files for the document viewer (the Python atm's `lib/documents.py`; docs/compmd-viewer-design.md §4):
//! which ones atm hands out, what each is, and the `CompMD` document the dashboard renders for it. atm renders
//! nothing itself: a code file becomes a fenced block of its language, an image a Markdown image, and `CompMD` does
//! the rest. The rules are here; the reading is the version control's: the working copy ([`read_document`]), or a
//! commit ([`read_document_at`]), with the same checks (I11, I12).
//!
//! A component of its own (docs/code-review-design.md §3.1, ATM-570): no task model in it. atm-lib re-exports it as
//! `atm_lib::documents`, the front ends' way in.

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
    let path = path_shape(path)?;
    if path.split('/').any(|s| s.starts_with('.')) {
        return Err(Error::Refused(format!(
            "{path}: hidden files and folders aren't shown"
        )));
    }
    Ok(path)
}

/// A file a review's round names, at its revision (ATM-598: a review's `.github/workflows/ci.yml` or `.gitignore`
/// showed no diff): checked as [`document_path`], but hidden files are shown: the review's own diff shows their
/// change, and the file is read from a commit or tree, never the working copy. Still refused: git's own folder and
/// atm's (`.git`, `.agentic-task-manager`), environment files (`.env`, `.env.local`, … but not `.env.example`,
/// `.env.sample` or `.env.template`) and key files.
///
/// # Errors
/// As [`document_path`].
pub fn revision_path(path: &str) -> Result<&str> {
    let path = path_shape(path)?;
    let refused = |what: &str| Err(Error::Refused(format!("{path}: {what} aren't shown")));
    let segments: Vec<&str> = path.split('/').collect();
    if segments
        .iter()
        .any(|s| matches!(*s, ".git" | ".agentic-task-manager"))
    {
        return refused("git's and atm's own folders");
    }
    let name = segments.last().copied().unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    let example = [".example", ".sample", ".template"]
        .iter()
        .any(|e| lower.ends_with(e));
    if (lower == ".env" || lower.starts_with(".env.")) && !example {
        return refused("environment files");
    }
    Ok(path)
}

/// A path's shape and its key-file check, common to both: relative, normalized, at most 512 characters, not a key file.
fn path_shape(path: &str) -> Result<&str> {
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
    let rendered = render(path, found.kind, found.detail, &data);
    #[allow(clippy::cast_possible_truncation)]
    // reason: nanoseconds since 1970 in seconds fit an i64
    let modified = format_iso((found.stamp.1 / 1_000_000_000) as i64);
    Ok(json!({
        "path": path,
        "kind": rendered.kind,
        "language": rendered.language,
        "text": rendered.text,
        "document": rendered.document,
        "line_offset": rendered.line_offset,
        "lines": rendered.lines,
        "bytes": data.len(),
        "modified_at": modified,
        "tracked": vcs.tracked(path),
        "commit": vcs.head(),
        // A file in a submodule lives in the submodule's repository: its folder, commit and web address (T-408)
        "submodule": vcs.submodule_of(path),
    }))
}

/// A file of the project as a commit or a tree has it, for the viewer (a review's round, a document's history): the
/// same fields as [`read_document`], with `revision` the commit's or tree's SHA (`commit` the commit's, none for a
/// tree), `modified_at` and `submodule` none, `tracked` true. The same checks: the path's, a regular file (no symlink, no submodule), not one git ignores
/// (by the working copy's rules), not too large.
///
/// # Errors
/// `Invalid` for a path or revision that isn't one; `NotFound` when the commit or the file isn't there; `Refused` and
/// `TooLarge` as [`read_document`].
pub fn read_document_at(vcs: Option<&GitVcs>, revision: &str, path: &str) -> Result<Value> {
    // A file a review names, at its revision: hidden ones too (ATM-598)
    let path = revision_path(path)?;
    let Some(vcs) = vcs else {
        return Err(Error::NotFound(
            "this project has no repository to read files from".into(),
        ));
    };
    // A revision is a commit's name, never an option for git
    if revision.is_empty() || revision.starts_with('-') || revision.contains('\0') {
        return Err(Error::Invalid(format!("{revision}: not a revision")));
    }
    // A commit, or a tree (a review's round rebuilt from its diff, which no commit has): the project's, or, for a file
    // in a submodule, the submodule's own (a review's round of commits made in it, ATM-583), read by its path there
    let found = |repo: &GitVcs| {
        let commit = repo.resolve(revision);
        let object = commit.clone().or_else(|| repo.resolve_tree(revision))?;
        Some((commit, object))
    };
    let (repo, inside, commit, object) = match found(vcs) {
        Some((commit, object)) => (vcs.clone(), path.to_string(), commit, object),
        None => vcs
            .in_submodule(path)
            .and_then(|(sub, inside)| found(&sub).map(|(c, o)| (sub, inside, c, o)))
            .ok_or_else(|| Error::NotFound(format!("{revision}: no such commit")))?,
    };
    let (size, blob) = repo
        .blob_in(&object, &inside)
        .ok_or_else(|| Error::NotFound(format!("{path}: not a file in {revision}")))?;
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
    if size > limit {
        return Err(Error::TooLarge(format!(
            "{path}: larger than {} MB",
            limit / 1_000_000
        )));
    }
    let data = repo.blob_bytes(&blob)?;
    let rendered = render(path, kind, detail, &data);
    Ok(json!({
        "path": path,
        "kind": rendered.kind,
        "language": rendered.language,
        "text": rendered.text,
        "document": rendered.document,
        "line_offset": rendered.line_offset,
        "lines": rendered.lines,
        "bytes": data.len(),
        "modified_at": null,
        "tracked": true,
        "commit": commit,
        "revision": object,
        "submodule": null,
    }))
}

/// A file's bytes as the viewer shows them.
struct Rendered {
    kind: &'static str,
    language: Option<&'static str>,
    text: Option<String>,
    document: String,
    line_offset: u32,
    lines: usize,
}

/// `data`, a file of the kind its name says, as the viewer shows it: a Markdown file as it is, code and text in a
/// fence, an image as a Markdown image of its own name, a binary file (or an image that isn't one) as a line saying so.
fn render(path: &str, kind: &'static str, detail: Option<&'static str>, data: &[u8]) -> Rendered {
    let name = path.rsplit('/').next().unwrap_or(path);
    let (mut kind, mut detail) = (kind, detail);
    let mut text = None;
    let mut line_offset = 0;
    let document = if kind == "image" {
        if image_matches(data, detail.unwrap_or_default()) {
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
        match std::str::from_utf8(data) {
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
    Rendered {
        kind,
        language: if kind == "code" { detail } else { None },
        text,
        document,
        line_offset,
        lines,
    }
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
    fn a_path_of_512_characters_is_taken_and_one_longer_is_invalid() {
        let at_limit = "a".repeat(DOC_MAX_PATH);
        assert!(document_path(&at_limit).is_ok());
        assert!(matches!(
            document_path(&format!("{at_limit}a")),
            Err(Error::Invalid(_))
        ));
        // Characters are counted, not bytes
        assert!(document_path(&"é".repeat(DOC_MAX_PATH)).is_ok());
    }

    #[test]
    fn a_path_that_isnt_a_normalized_relative_one_is_invalid() {
        for bad in ["a\0b", "a/./b", "./a", "a/", "..", "/", "a/b/.."] {
            assert!(
                matches!(document_path(bad), Err(Error::Invalid(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_hidden_segment_anywhere_in_the_path_is_refused() {
        for hidden in [
            ".agentic-task-manager/tasks.json",
            "docs/.hidden/x.md",
            "src/.env.local",
            ".github/workflows/ci.yml",
        ] {
            assert!(
                matches!(document_path(hidden), Err(Error::Refused(_))),
                "{hidden}"
            );
        }
    }

    /// ATM-598: a file a review names, at its revision: hidden ones shown, but not git's or atm's folders, environment
    /// files or key files; its shape checked as any path's
    #[test]
    fn a_hidden_file_at_a_revision_is_shown_but_not_gits_atms_environments_or_keys() {
        for shown in [
            ".github/workflows/ci.yml",
            ".gitignore",
            ".gitattributes",
            "docs/.hidden/x.md",
            ".env.example",
            "app/.env.sample",
            ".env.TEMPLATE",
            ".prettierrc",
            "src/.environment.ts",
        ] {
            assert_eq!(revision_path(shown).ok(), Some(shown), "{shown}");
        }
        for refused in [
            ".git/config",
            "sub/.git/HEAD",
            ".agentic-task-manager/tasks.json",
            ".env",
            "src/.env.local",
            ".ENV.production",
            ".ssh/id_rsa",
            "certs/.server.pem",
        ] {
            assert!(
                matches!(revision_path(refused), Err(Error::Refused(_))),
                "{refused}"
            );
        }
        for bad in ["", "/etc/passwd", "a/../b", "a\\b", "./a"] {
            assert!(
                matches!(revision_path(bad), Err(Error::Invalid(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn every_key_file_name_is_refused_in_any_case_and_only_by_its_name() {
        for key in [
            "a.pem",
            "a.key",
            "a.p12",
            "a.pfx",
            "a.keystore",
            "a.jks",
            "certs/SERVER.KEY",
            "id_rsa",
            "id_dsa",
            "id_ecdsa",
            "ssh/id_ed25519",
            "ID_RSA.pub",
            "id_ecdsa.old",
        ] {
            assert!(
                matches!(document_path(key), Err(Error::Refused(_))),
                "{key}"
            );
        }
        // Only the file's own name: a folder named like a key, a key's name inside another name, are shown
        for shown in [
            "x.pem/notes.md",
            "a.pem.md",
            "keys.md",
            "pem",
            "id_rsa_backup",
            "my_id_rsa",
        ] {
            assert!(document_path(shown).is_ok(), "{shown}");
        }
    }

    #[test]
    fn every_kind_of_file_is_known_by_its_last_segments_extension_or_whole_name() {
        for (path, kind) in [
            ("README.MD", ("markdown", None)),
            ("notes.markdown", ("markdown", None)),
            ("a.png", ("image", Some("image/png"))),
            ("a.JPG", ("image", Some("image/jpeg"))),
            ("a.jpeg", ("image", Some("image/jpeg"))),
            ("a.gif", ("image", Some("image/gif"))),
            ("a.webp", ("image", Some("image/webp"))),
            ("web/App.svelte", ("code", Some("svelte"))),
            ("Cargo.toml", ("code", Some("toml"))),
            ("src/Makefile", ("code", Some("makefile"))),
            ("fix.patch", ("code", Some("diff"))),
            // The last dot's part: a double extension is its last one
            ("a.tar.gz", ("text", None)),
            ("a.test.ts", ("code", Some("ts"))),
            // No extension, no name atm knows: text
            ("LICENSE", ("text", None)),
            // A leading dot isn't an extension's
            (".bashrc", ("text", None)),
            // The extension of the file, not of a folder on its way
            ("src.rs/notes", ("text", None)),
            // A whole name is matched as it is written
            ("makefile", ("text", None)),
        ] {
            assert_eq!(document_kind(path), kind, "{path}");
        }
    }

    #[test]
    fn a_fence_holds_any_text_with_its_own_last_line_end() {
        for (text, language, fenced) in [
            ("", None, "```\n```\n"),
            ("x", Some("rust"), "```rust\nx\n```\n"),
            ("x\n", None, "```\nx\n```\n"),
            // Runs of backticks shorter than three keep the usual fence; three or more get one longer
            ("a `` b\n", None, "```\na `` b\n```\n"),
            ("```\n", None, "````\n```\n````\n"),
            ("` `` ```", None, "````\n` `` ```\n````\n"),
        ] {
            assert_eq!(fence(text, language), fenced, "{text:?}");
        }
    }

    #[test]
    fn an_image_matches_only_the_bytes_its_type_starts_with() {
        let mut far_svg = vec![b' '; DOC_SNIFF_BYTES];
        far_svg.extend_from_slice(b"<svg>");
        let cases: [(&[u8], &str, bool); 15] = [
            (b"\x89PNG\r\n\x1a\nrest", "image/png", true),
            (b"\x89PNG", "image/png", false),
            (b"\xff\xd8\xff\xe0", "image/jpeg", true),
            (b"\xff\xd8", "image/jpeg", false),
            (b"GIF87a", "image/gif", true),
            (b"GIF88a", "image/gif", false),
            (b"RIFF\0\0\0\0WEBPVP8 ", "image/webp", true),
            (b"RIFF\0\0\0\0WAVE", "image/webp", false),
            (b"RIFF", "image/webp", false),
            (
                b"<?xml version=\"1.0\"?>\n<svg xmlns=\"\">",
                "image/svg+xml",
                true,
            ),
            (b"<SVG>", "image/svg+xml", true),
            (b"<html></html>", "image/svg+xml", false),
            (&far_svg, "image/svg+xml", false),
            (b"", "image/png", false),
            // A type the viewer doesn't show matches nothing
            (b"BM", "image/bmp", false),
        ];
        for (data, content_type, matches) in cases {
            assert_eq!(
                image_matches(data, content_type),
                matches,
                "{content_type} {:?}",
                String::from_utf8_lossy(&data[..data.len().min(24)])
            );
        }
    }

    #[test]
    fn a_link_target_is_bracketed_only_when_markdown_would_misread_it() {
        for (name, target) in [
            ("a.png", "a.png"),
            ("a-b_c.png", "a-b_c.png"),
            ("a b.png", "<a b.png>"),
            ("a\tb.png", "<a\tb.png>"),
            ("a(1).png", "<a(1).png>"),
            ("a)b.png", "<a)b.png>"),
            ("a<b>.png", "<a<b>.png>"),
        ] {
            assert_eq!(link_target(name), target, "{name}");
        }
    }

    #[test]
    fn markdown_is_rendered_as_it_is_and_code_and_text_in_a_fence() {
        let markdown = render("docs/a.md", "markdown", None, b"# A\n\nText.\n");
        assert_eq!(
            (
                markdown.kind,
                markdown.language,
                markdown.document.as_str(),
                markdown.line_offset,
                markdown.lines
            ),
            ("markdown", None, "# A\n\nText.\n", 0, 3)
        );
        assert_eq!(markdown.text.as_deref(), Some("# A\n\nText.\n"));
        let code = render("a.rs", "code", Some("rust"), b"fn a() {}\nfn b() {}");
        assert_eq!(
            (
                code.kind,
                code.language,
                code.document.as_str(),
                code.line_offset,
                code.lines
            ),
            (
                "code",
                Some("rust"),
                "```rust\nfn a() {}\nfn b() {}\n```\n",
                1,
                2
            )
        );
        // Text has no language; an empty file no lines
        let text = render("notes.txt", "text", None, b"");
        assert_eq!(
            (text.kind, text.language, text.document.as_str(), text.lines),
            ("text", None, "```\n```\n", 0)
        );
    }

    #[test]
    fn a_file_that_isnt_utf8_text_is_rendered_as_a_line_saying_its_binary() {
        let nul = render("a.rs", "code", Some("rust"), b"fn\0");
        assert_eq!(
            (nul.kind, nul.language, nul.text, nul.document.as_str()),
            ("binary", None, None, "*a.rs is a binary file: 3 bytes.*\n")
        );
        assert_eq!((nul.lines, nul.line_offset), (0, 0));
        let latin1 = render("dir/b.md", "markdown", None, b"caf\xe9");
        assert_eq!(
            (latin1.kind, latin1.document.as_str()),
            ("binary", "*b.md is a binary file: 4 bytes.*\n")
        );
        // A NUL past what is sniffed doesn't make a file binary
        let mut late = vec![b'a'; DOC_SNIFF_BYTES];
        late.push(0);
        assert_eq!(render("a.txt", "text", None, &late).kind, "text");
    }

    #[test]
    fn an_image_is_rendered_as_a_markdown_image_of_itself_or_binary_when_it_isnt_one() {
        let png = b"\x89PNG\r\n\x1a\n";
        let image = render("img/a b.png", "image", Some("image/png"), png);
        assert_eq!(
            (
                image.kind,
                image.language,
                image.text,
                image.document.as_str()
            ),
            ("image", None, None, "![a b.png](<a b.png>)\n")
        );
        let fake = render("a.png", "image", Some("image/png"), b"<html>");
        assert_eq!(
            (fake.kind, fake.document.as_str()),
            (
                "binary",
                "*a.png isn't the image its name says: 6 bytes.*\n"
            )
        );
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

    /// A throwaway repository: `a.md` committed twice (`one`, then `two`), the first commit tagged `first`.
    struct Repo(std::path::PathBuf);

    impl Repo {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("atm-docs-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let repo = Self(dir);
            repo.git(&["init", "-q"]);
            repo.git(&["config", "user.email", "t@example.invalid"]);
            repo.git(&["config", "user.name", "t"]);
            repo.commit("a.md", "# One\n");
            repo.git(&["tag", "first"]);
            repo.commit("a.md", "# Two\n");
            repo
        }

        fn git(&self, args: &[&str]) {
            assert!(atm_fs::git::git(args, &self.0).is_some(), "git {args:?}");
        }

        fn commit(&self, file: &str, text: &str) {
            std::fs::write(self.0.join(file), text).unwrap();
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", file]);
        }

        fn vcs(&self) -> GitVcs {
            GitVcs::new(&self.0)
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// ATM-583: a review's round of commits made in a submodule names them by the submodule's commits (or trees)
    #[test]
    fn a_file_in_a_submodule_at_one_of_its_revisions_is_read_from_its_own_repository() {
        let sub = Repo::new();
        let project = Repo::new();
        let source = sub.0.to_string_lossy().into_owned();
        project.git(&[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            &source,
            "lib",
        ]);
        project.git(&["commit", "-q", "-m", "add lib"]);
        let lib = Repo(project.0.join("lib"));
        lib.git(&["config", "user.email", "t@example.invalid"]);
        lib.git(&["config", "user.name", "t"]);
        lib.commit("a.md", "# Three, in lib\n");
        let head = atm_fs::git::git(&["rev-parse", "HEAD"], &lib.0).unwrap();
        let tree = atm_fs::git::git(&["rev-parse", "HEAD^{tree}"], &lib.0).unwrap();
        let vcs = project.vcs();
        let at_commit = read_document_at(Some(&vcs), &head, "lib/a.md").unwrap();
        assert_eq!(at_commit["text"], json!("# Three, in lib\n"));
        assert_eq!(at_commit["path"], json!("lib/a.md"));
        assert_eq!(at_commit["commit"], json!(head));
        let at_tree = read_document_at(Some(&vcs), &tree, "lib/a.md").unwrap();
        assert_eq!(at_tree["text"], json!("# Three, in lib\n"));
        // The project's own revisions still read the project's files; the submodule's name none of them
        assert_eq!(
            read_document_at(Some(&vcs), "first", "a.md").unwrap()["text"],
            json!("# One\n")
        );
        assert!(matches!(
            read_document_at(Some(&vcs), &head, "a.md"),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            read_document_at(Some(&vcs), &head, "lib/none.md"),
            Err(Error::NotFound(_))
        ));
        std::mem::forget(lib); // inside the project, which removes it
    }

    #[test]
    fn a_file_at_a_revision_is_read_as_that_commit_has_it() {
        let repo = Repo::new();
        let vcs = repo.vcs();
        let then = read_document_at(Some(&vcs), "first", "a.md").unwrap();
        assert_eq!(then["document"], "# One\n");
        assert_eq!(then["kind"], "markdown");
        assert_eq!(then["revision"], json!(vcs.resolve("first")));
        assert_eq!(
            read_document(Some(&vcs), "a.md").unwrap()["document"],
            "# Two\n"
        );
        // Code is fenced as it is from the working copy
        repo.commit("src.rs", "fn main() {}\n");
        let code = read_document_at(Some(&vcs), "HEAD", "src.rs").unwrap();
        assert_eq!(
            (code["language"].as_str(), code["line_offset"].as_u64()),
            (Some("rust"), Some(1))
        );
    }

    /// A review's round rebuilt from its diff is a tree no commit has: read the same way, with no commit named
    #[test]
    fn a_file_in_a_tree_is_read_as_the_tree_has_it() {
        let repo = Repo::new();
        let vcs = repo.vcs();
        let tree = vcs.resolve_tree("first").unwrap();
        let then = read_document_at(Some(&vcs), &tree, "a.md").unwrap();
        assert_eq!(then["document"], "# One\n");
        assert_eq!(then["revision"], json!(tree));
        assert_eq!(then["commit"], Value::Null);
        // A commit's own answer names the commit
        let at_commit = read_document_at(Some(&vcs), "first", "a.md").unwrap();
        assert_eq!(at_commit["commit"], json!(vcs.resolve("first")));
        // The tree's checks are a commit's: a file it doesn't have, a path atm doesn't show
        assert!(matches!(
            read_document_at(Some(&vcs), &tree, "b.md"),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            read_document_at(Some(&vcs), &tree, ".env"),
            Err(Error::Refused(_))
        ));
    }

    /// ATM-598: a hidden file a commit has is read at it (a review's side); the working copy's never is
    #[test]
    fn a_hidden_file_is_read_at_a_revision_but_not_from_the_working_copy() {
        let repo = Repo::new();
        let vcs = repo.vcs();
        std::fs::create_dir_all(repo.0.join(".github/workflows")).unwrap();
        repo.commit(".github/workflows/ci.yml", "on: push\n");
        let side = read_document_at(Some(&vcs), "HEAD", ".github/workflows/ci.yml").unwrap();
        assert_eq!(side["text"], "on: push\n");
        assert!(matches!(
            read_document(Some(&vcs), ".github/workflows/ci.yml"),
            Err(Error::Refused(_))
        ));
        // An environment file committed by mistake stays refused at a revision too
        repo.commit(".env", "TOKEN=x\n");
        assert!(matches!(
            read_document_at(Some(&vcs), "HEAD", ".env"),
            Err(Error::Refused(_))
        ));
    }

    #[test]
    fn a_file_at_a_revision_passes_the_same_checks() {
        let repo = Repo::new();
        let vcs = repo.vcs();
        // The path's checks first, whatever the commit
        assert!(matches!(
            read_document_at(Some(&vcs), "HEAD", ".env"),
            Err(Error::Refused(_))
        ));
        assert!(matches!(
            read_document_at(Some(&vcs), "HEAD", "../a.md"),
            Err(Error::Invalid(_))
        ));
        // A revision is never an option for git
        for bad in ["-p", "--output=x", ""] {
            assert!(
                matches!(
                    read_document_at(Some(&vcs), bad, "a.md"),
                    Err(Error::Invalid(_))
                ),
                "{bad}"
            );
        }
        assert!(matches!(
            read_document_at(Some(&vcs), "no-such-ref", "a.md"),
            Err(Error::NotFound(_))
        ));
        // Not in that commit; a folder isn't a file
        assert!(matches!(
            read_document_at(Some(&vcs), "first", "b.md"),
            Err(Error::NotFound(_))
        ));
        std::fs::create_dir_all(repo.0.join("docs")).unwrap();
        repo.commit("docs/x.md", "x\n");
        assert!(matches!(
            read_document_at(Some(&vcs), "HEAD", "docs"),
            Err(Error::NotFound(_))
        ));
        // A file the ignore rules name: as the working copy answers (git ignores no file it tracks), whether the commit
        // has it or not
        std::fs::write(repo.0.join(".gitignore"), "secret.txt\nlocal.md\n").unwrap();
        std::fs::write(repo.0.join("secret.txt"), "s\n").unwrap();
        repo.git(&["add", "-f", "secret.txt", ".gitignore"]);
        repo.git(&["commit", "-q", "-m", "secret"]);
        assert_eq!(
            read_document_at(Some(&vcs), "HEAD", "secret.txt").is_ok(),
            read_document(Some(&vcs), "secret.txt").is_ok()
        );
        // Ignored and untracked now, committed in an older commit (before the rule): refused, as from the working copy
        std::fs::write(repo.0.join("local.md"), "l\n").unwrap();
        repo.git(&["add", "-f", "local.md"]);
        repo.git(&["commit", "-q", "-m", "local"]);
        repo.git(&["rm", "-q", "--cached", "local.md"]);
        repo.git(&["commit", "-q", "-m", "untrack"]);
        assert!(matches!(
            read_document_at(Some(&vcs), "HEAD~1", "local.md"),
            Err(Error::Refused(_))
        ));
        assert!(matches!(
            read_document(Some(&vcs), "local.md"),
            Err(Error::Refused(_))
        ));
    }

    #[test]
    fn without_a_working_copy_nothing_is_read_but_a_bad_path_is_still_named() {
        assert!(matches!(
            read_document(None, "a.md"),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            document_stamp(None, "a.md"),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(read_image(None, "a.png"), Err(Error::NotFound(_))));
        assert!(matches!(
            read_document_at(None, "HEAD", "a.md"),
            Err(Error::NotFound(_))
        ));
        // The path's checks come first
        assert!(matches!(
            read_document(None, ".env"),
            Err(Error::Refused(_))
        ));
        assert!(matches!(
            read_document_at(None, "HEAD", "../a"),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn a_working_copy_file_is_read_with_its_size_time_tracking_and_commit() {
        let repo = Repo::new();
        let vcs = repo.vcs();
        let tracked = read_document(Some(&vcs), "a.md").unwrap();
        assert_eq!(tracked["bytes"], 6);
        assert_eq!(tracked["tracked"], true);
        assert_eq!(tracked["commit"], json!(vcs.head()));
        assert_eq!(tracked["submodule"], Value::Null);
        assert!(
            tracked["modified_at"]
                .as_str()
                .is_some_and(|t| t.starts_with("20")),
            "{}",
            tracked["modified_at"]
        );
        std::fs::write(repo.0.join("new.txt"), "n\n").unwrap();
        let untracked = read_document(Some(&vcs), "new.txt").unwrap();
        assert_eq!(untracked["tracked"], false);
        assert_eq!(untracked["kind"], "text");
        // The stamp is the file's size, from the same checks
        assert_eq!(document_stamp(Some(&vcs), "new.txt").unwrap().0, 2);
        assert!(matches!(
            document_stamp(Some(&vcs), ".env"),
            Err(Error::Refused(_))
        ));
    }

    #[test]
    fn a_working_copy_path_that_isnt_a_file_there_is_not_found() {
        let repo = Repo::new();
        let vcs = repo.vcs();
        std::fs::create_dir_all(repo.0.join("docs")).unwrap();
        for missing in ["b.md", "docs", "docs/none.md"] {
            assert!(
                matches!(read_document(Some(&vcs), missing), Err(Error::NotFound(_))),
                "{missing}"
            );
        }
    }

    #[test]
    fn a_file_git_ignores_is_refused_from_the_working_copy() {
        let repo = Repo::new();
        let vcs = repo.vcs();
        std::fs::write(repo.0.join(".gitignore"), "build/\n").unwrap();
        std::fs::create_dir_all(repo.0.join("build")).unwrap();
        std::fs::write(repo.0.join("build/out.txt"), "o\n").unwrap();
        assert!(matches!(
            read_document(Some(&vcs), "build/out.txt"),
            Err(Error::Refused(_))
        ));
        assert!(matches!(
            document_stamp(Some(&vcs), "build/out.txt"),
            Err(Error::Refused(_))
        ));
    }

    #[test]
    fn a_document_up_to_1_mb_and_an_image_up_to_5_mb_are_read_and_larger_ones_refused() {
        let repo = Repo::new();
        let vcs = repo.vcs();
        let limit = usize::try_from(DOC_MAX_BYTES).unwrap();
        let image_limit = usize::try_from(DOC_MAX_IMAGE_BYTES).unwrap();
        std::fs::write(repo.0.join("at.txt"), vec![b'a'; limit]).unwrap();
        std::fs::write(repo.0.join("past.txt"), vec![b'a'; limit + 1]).unwrap();
        assert!(read_document(Some(&vcs), "at.txt").is_ok());
        assert!(matches!(
            read_document(Some(&vcs), "past.txt"),
            Err(Error::TooLarge(_))
        ));
        assert!(matches!(
            document_stamp(Some(&vcs), "past.txt"),
            Err(Error::TooLarge(_))
        ));
        // An image has a limit of its own
        let png = |len: usize| {
            let mut data = b"\x89PNG\r\n\x1a\n".to_vec();
            data.resize(len, 0);
            data
        };
        std::fs::write(repo.0.join("big.png"), png(limit + 1)).unwrap();
        std::fs::write(repo.0.join("at.png"), png(image_limit)).unwrap();
        std::fs::write(repo.0.join("past.png"), png(image_limit + 1)).unwrap();
        assert_eq!(
            read_document(Some(&vcs), "big.png").unwrap()["kind"],
            "image"
        );
        assert!(read_image(Some(&vcs), "at.png").is_ok());
        assert!(matches!(
            read_image(Some(&vcs), "past.png"),
            Err(Error::TooLarge(_))
        ));
        // As at a revision
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "-q", "-m", "large"]);
        assert!(matches!(
            read_document_at(Some(&vcs), "HEAD", "past.txt"),
            Err(Error::TooLarge(_))
        ));
        assert!(matches!(
            read_document_at(Some(&vcs), "HEAD", "past.png"),
            Err(Error::TooLarge(_))
        ));
        assert!(read_document_at(Some(&vcs), "HEAD", "big.png").is_ok());
    }

    #[test]
    fn an_image_file_is_served_only_when_its_bytes_are_the_image_its_name_says() {
        let repo = Repo::new();
        let vcs = repo.vcs();
        std::fs::write(repo.0.join("a.gif"), b"GIF89a....").unwrap();
        std::fs::write(repo.0.join("fake.png"), b"<script>").unwrap();
        assert_eq!(
            read_image(Some(&vcs), "a.gif").unwrap(),
            (b"GIF89a....".to_vec(), "image/gif")
        );
        assert!(matches!(
            read_image(Some(&vcs), "fake.png"),
            Err(Error::Refused(_))
        ));
        // Anything but an image, even one the viewer shows, isn't served as a file
        assert!(matches!(
            read_image(Some(&vcs), "a.md"),
            Err(Error::Refused(_))
        ));
        assert!(matches!(
            read_image(Some(&vcs), "none.png"),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            read_image(Some(&vcs), ".hidden.png"),
            Err(Error::Refused(_))
        ));
    }

    #[test]
    fn outside_git_a_file_is_read_with_no_tracking_and_no_commit() {
        let dir = std::env::temp_dir().join(format!("atm-docs-plain-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "# A\n").unwrap();
        let vcs = GitVcs::new(&dir);
        let read = read_document(Some(&vcs), "a.md");
        let _ = std::fs::remove_dir_all(&dir);
        let read = read.unwrap();
        assert_eq!(read["document"], "# A\n");
        assert_eq!(
            (&read["tracked"], &read["commit"]),
            (&Value::Null, &Value::Null)
        );
    }

    #[test]
    fn a_revision_with_a_nul_is_invalid_and_a_binary_file_in_a_commit_says_so() {
        let repo = Repo::new();
        let vcs = repo.vcs();
        assert!(matches!(
            read_document_at(Some(&vcs), "HEAD\0", "a.md"),
            Err(Error::Invalid(_))
        ));
        std::fs::write(repo.0.join("data.txt"), b"a\0b").unwrap();
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "-q", "-m", "binary"]);
        let binary = read_document_at(Some(&vcs), "HEAD", "data.txt").unwrap();
        assert_eq!(binary["kind"], "binary");
        assert_eq!(binary["text"], Value::Null);
        assert_eq!(binary["bytes"], 3);
        assert_eq!(binary["tracked"], true);
        assert_eq!(binary["modified_at"], Value::Null);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_in_the_working_copy_is_not_a_file_nor_a_folder_on_the_way_to_one() {
        let repo = Repo::new();
        let vcs = repo.vcs();
        std::os::unix::fs::symlink(repo.0.join("a.md"), repo.0.join("link.md")).unwrap();
        std::fs::create_dir_all(repo.0.join("real")).unwrap();
        std::fs::write(repo.0.join("real/b.md"), "b\n").unwrap();
        std::os::unix::fs::symlink(repo.0.join("real"), repo.0.join("via")).unwrap();
        for linked in ["link.md", "via/b.md"] {
            assert!(
                matches!(read_document(Some(&vcs), linked), Err(Error::NotFound(_))),
                "{linked}"
            );
        }
        assert!(read_document(Some(&vcs), "real/b.md").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_in_a_commit_is_not_a_file() {
        let repo = Repo::new();
        std::os::unix::fs::symlink("/etc/passwd", repo.0.join("link.md")).unwrap();
        repo.git(&["add", "link.md"]);
        repo.git(&["commit", "-q", "-m", "link"]);
        assert!(matches!(
            read_document_at(Some(&repo.vcs()), "HEAD", "link.md"),
            Err(Error::NotFound(_))
        ));
    }
}
