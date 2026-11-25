//! The listing's rules (ATM-764, docs/documents-design.md §2–§3): which files, their titles, submodules, the cap and
//! the query, without git.

use super::*;

/// A throwaway repository, empty until a test writes to it.
struct Repo(std::path::PathBuf);

impl Repo {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("atm-docs-listing-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Self(dir);
        repo.git(&["init", "-q"]);
        repo.git(&["config", "user.email", "t@example.invalid"]);
        repo.git(&["config", "user.name", "t"]);
        repo
    }

    fn git(&self, args: &[&str]) {
        assert!(atm_fs::git::git(args, &self.0).is_some(), "git {args:?}");
    }

    fn write(&self, file: &str, text: &str) {
        let path = self.0.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn commit(&self, files: &[(&str, &str)]) {
        for (file, text) in files {
            self.write(file, text);
        }
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "files"]);
    }

    /// `sub` added as a submodule at `folder`, and committed
    fn add_submodule(&self, sub: &Repo, folder: &str) {
        let source = sub.0.to_string_lossy().into_owned();
        self.git(&[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            &source,
            folder,
        ]);
        self.git(&["commit", "-q", "-m", "add submodule"]);
    }

    fn vcs(&self) -> GitVcs {
        GitVcs::new(&self.0)
    }

    fn list(&self, query: &str) -> Value {
        list_documents(Some(&self.vcs()), query).unwrap()
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The paths a listing gives, in its order
fn paths(listing: &Value) -> Vec<String> {
    listing["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["path"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_title_is_the_first_level_one_heading_atx_or_setext_else_the_files_name() {
    let cases: &[(&str, &str)] = &[
        ("# The design\n\ntext", "The design"),
        ("  #   Spaced out  \n", "Spaced out"),
        ("# Closed #\n", "Closed"),
        ("# Ends in C#\n", "Ends in C#"),
        ("Intro line\n\nThe title\n=====\n", "The title"),
        ("The title\n===  \n", "The title"),
        // Only level one: a level-two heading, a Setext `---` and a `#hashtag` aren't titles
        ("## Not it\n\nSub\n---\n#tag\n", "notes"),
        // Front matter first, then the heading
        ("---\ntitle: x\n# not a heading\n---\n# After\n", "After"),
        // A heading in fenced code isn't the document's
        ("```sh\n# a comment\n```\n# Real\n", "Real"),
        ("~~~\n# a comment\n~~~\n\n# Real\n", "Real"),
        // Indented four spaces: code, not a heading
        ("    # code\n", "notes"),
        ("#\n# \n", "notes"),
        ("", "notes"),
    ];
    for (text, title) in cases {
        assert_eq!(
            document_title(text.as_bytes(), "docs/notes.md"),
            *title,
            "{text:?}"
        );
    }
    assert_eq!(document_title(b"", "README.markdown"), "README");
    assert_eq!(document_title(b"", "a.b.MD"), "a.b");
    // Capped
    let long = format!("# {}\n", "x".repeat(300));
    let title = document_title(long.as_bytes(), "a.md");
    assert_eq!(title.chars().count(), 200);
    assert!(title.ends_with('…'));
}

#[test]
fn the_documents_are_the_markdown_files_git_tracks_or_has_untracked_and_not_ignored() {
    let repo = Repo::new();
    repo.commit(&[
        ("README.md", "# The project\n"),
        ("docs/design.markdown", "Design\n===\n"),
        ("src/main.rs", "fn main() {}\n"),
        (".gitignore", "build/\n*.tmp.md\n"),
        (".github/CONTRIBUTING.md", "# Contributing\n"),
    ]);
    repo.write("docs/draft.md", "# A draft\n");
    repo.write("docs/scratch.tmp.md", "# Ignored\n");
    repo.write("build/out.md", "# Ignored too\n");
    repo.write("notes.txt", "not markdown\n");
    let listing = repo.list("");
    assert_eq!(listing["git"], json!(true));
    assert_eq!(
        paths(&listing),
        [
            ".github/CONTRIBUTING.md",
            "README.md",
            "docs/design.markdown",
            "docs/draft.md"
        ]
    );
    let entry = |path: &str| {
        listing["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["path"] == path)
            .unwrap()
            .clone()
    };
    let readme = entry("README.md");
    assert_eq!(readme["title"], json!("The project"));
    assert_eq!(readme["tracked"], json!(true));
    assert_eq!(readme["bytes"], json!(14));
    assert!(readme["modified_at"].as_str().unwrap().ends_with('Z'));
    assert_eq!(entry("docs/design.markdown")["title"], json!("Design"));
    assert_eq!(entry("docs/draft.md")["tracked"], json!(false));
    assert_eq!(listing["more"], json!(0));
    assert_eq!(listing["submodules"], json!([]));
}

#[test]
fn what_the_viewer_refuses_is_not_listed_nor_a_symlink() {
    let repo = Repo::new();
    repo.commit(&[
        ("a.md", "# A\n"),
        (".agentic-task-manager/notes.md", "# atm's\n"),
    ]);
    // An environment file named like a document, and a link to a document
    repo.write(".env.md", "# not env\n");
    #[cfg(unix)]
    std::os::unix::fs::symlink(repo.0.join("a.md"), repo.0.join("link.md")).unwrap();
    let listing = repo.list("");
    let listed = paths(&listing);
    assert!(listed.contains(&"a.md".to_string()));
    assert!(
        !listed
            .iter()
            .any(|p| p.starts_with(".agentic-task-manager/")),
        "{listed:?}"
    );
    assert!(!listed.contains(&"link.md".to_string()), "{listed:?}");
    // Each one listed is one the viewer reads
    for path in &listed {
        assert!(crate::document_path(path).is_ok(), "{path}");
    }
}

#[test]
fn a_submodules_documents_are_listed_from_its_own_repository_nested_ones_too_one_not_checked_out_says_so()
 {
    let inner = Repo::new();
    inner.commit(&[("deep.md", "# Deep\n")]);
    let sub = Repo::new();
    sub.commit(&[
        ("guide.md", "# The guide\n"),
        (".gitignore", "*.draft.md\n"),
    ]);
    sub.add_submodule(&inner, "inner");
    let project = Repo::new();
    project.commit(&[("README.md", "# Project\n")]);
    project.add_submodule(&sub, "lib");
    project.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "update",
        "--init",
        "--recursive",
        "-q",
    ]);
    // Untracked in the submodule: listed by its own rules (its ignored one left out)
    project.write("lib/new.md", "# New in lib\n");
    project.write("lib/x.draft.md", "# Ignored in lib\n");
    let listing = project.list("");
    assert_eq!(
        paths(&listing),
        [
            "README.md",
            "lib/guide.md",
            "lib/inner/deep.md",
            "lib/new.md"
        ]
    );
    let new = &listing["documents"][3];
    assert_eq!(
        (new["title"].clone(), new["tracked"].clone()),
        (json!("New in lib"), json!(false))
    );
    assert_eq!(
        listing["submodules"],
        json!([{"path": "lib", "checked_out": true}, {"path": "lib/inner", "checked_out": true}])
    );

    // Not checked out: listed as such, with none of its documents
    let other = Repo::new();
    other.commit(&[("README.md", "# Other\n")]);
    other.add_submodule(&sub, "lib");
    other.git(&["submodule", "deinit", "-q", "-f", "lib"]);
    let listing = other.list("");
    assert_eq!(paths(&listing), ["README.md"]);
    assert_eq!(
        listing["submodules"],
        json!([{"path": "lib", "checked_out": false}])
    );
}

#[test]
fn at_most_500_by_path_and_how_many_more_the_query_matching_before_the_cut() {
    let repo = Repo::new();
    let files: Vec<(String, String)> = (0..503)
        .map(|i| (format!("d/{i:04}.md"), format!("# Note {i}\n")))
        .chain([("z/last.md".to_string(), "# The needle\n".to_string())])
        .collect();
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect();
    repo.commit(&refs);
    let listing = repo.list("");
    let listed = paths(&listing);
    assert_eq!(listed.len(), LIST_MAX);
    assert_eq!(
        (listed[0].as_str(), listed[499].as_str()),
        ("d/0000.md", "d/0499.md")
    );
    assert_eq!(listing["more"], json!(4));
    // The last by path, past the cut: found by its title, any case, and by its path
    let found = repo.list("NEEDLE");
    assert_eq!(paths(&found), ["z/last.md"]);
    assert_eq!(found["more"], json!(0));
    assert_eq!(paths(&repo.list("z/ the")), ["z/last.md"]);
    // Every word must be there
    assert_eq!(paths(&repo.list("needle missing")), Vec::<String>::new());
    // Many matches: cut too
    assert_eq!(repo.list("note")["more"], json!(3));
}

#[test]
fn a_query_longer_than_200_characters_is_refused() {
    let repo = Repo::new();
    assert!(matches!(
        list_documents(Some(&repo.vcs()), &"x".repeat(201)),
        Err(Error::Invalid(_))
    ));
    assert!(list_documents(Some(&repo.vcs()), &"x".repeat(200)).is_ok());
}

#[test]
fn without_git_there_are_no_documents_and_no_stamp() {
    let dir = std::env::temp_dir().join(format!("atm-docs-listing-nogit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.md"), "# A\n").unwrap();
    let vcs = GitVcs::new(&dir);
    let none = json!({"git": false, "documents": [], "submodules": [], "more": 0});
    // A folder outside any repository (the temp dir may itself be inside one: then skip)
    if !vcs.in_repository() {
        assert_eq!(list_documents(Some(&vcs), "").unwrap(), none);
        assert_eq!(documents_stamp(Some(&vcs)), None);
    }
    assert_eq!(list_documents(None, "").unwrap(), none);
    assert_eq!(documents_stamp(None), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_stamp_moves_with_a_document_changed_added_or_committed_and_only_then() {
    let repo = Repo::new();
    repo.commit(&[("a.md", "# A\n"), ("code.rs", "")]);
    let vcs = repo.vcs();
    let first = documents_stamp(Some(&vcs)).unwrap();
    assert_eq!(documents_stamp(Some(&vcs)).unwrap(), first);
    // Not a document: the same
    repo.write("code.rs", "fn x() {}\n");
    assert_eq!(documents_stamp(Some(&vcs)).unwrap(), first);
    repo.write("b.md", "# B\n");
    let added = documents_stamp(Some(&vcs)).unwrap();
    assert_ne!(added, first);
    repo.git(&["add", "b.md"]);
    let staged = documents_stamp(Some(&vcs)).unwrap();
    assert_ne!(staged, added, "tracked now");
    repo.git(&["commit", "-q", "-m", "b"]);
    assert_ne!(documents_stamp(Some(&vcs)).unwrap(), staged, "a new HEAD");
}
