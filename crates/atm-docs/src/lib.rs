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
    let path = document_path(path)?;
    let Some(vcs) = vcs else {
        return Err(Error::NotFound(
            "this project has no repository to read files from".into(),
        ));
    };
    // A revision is a commit's name, never an option for git
    if revision.is_empty() || revision.starts_with('-') || revision.contains('\0') {
        return Err(Error::Invalid(format!("{revision}: not a revision")));
    }
    // A commit, or a tree (a review's round rebuilt from its diff, which no commit has)
    let commit = vcs.resolve(revision);
    let object = match &commit {
        Some(commit) => commit.clone(),
        None => vcs
            .resolve_tree(revision)
            .ok_or_else(|| Error::NotFound(format!("{revision}: no such commit")))?,
    };
    let (size, blob) = vcs
        .blob_in(&object, path)
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
    let data = vcs.blob_bytes(&blob)?;
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
