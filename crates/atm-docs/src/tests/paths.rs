//! The paths atm-docs takes: their shape, hidden files, key files, and a file's kind by its name.

use crate::*;

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

/// ATM-598, CR-3: hidden files shown, in the working copy and at a revision, but not git's or atm's folders,
/// environment files or key files; its shape checked as any path's
#[test]
fn a_hidden_file_is_shown_but_not_gits_atms_environments_or_keys() {
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
        assert_eq!(document_path(shown).ok(), Some(shown), "{shown}");
    }
    for refused in [
        ".git/config",
        "sub/.git/HEAD",
        ".agentic-task-manager/tasks.json",
        // In any case, and as Windows reads trailing dots and spaces (ATM-671)
        ".GIT/config",
        "sub/.Git/HEAD",
        ".Agentic-Task-Manager/tasks.json",
        ".git./config",
        ".git /config",
        ".env",
        "src/.env.local",
        ".ENV.production",
        ".ssh/id_rsa",
        "certs/.server.pem",
    ] {
        assert!(
            matches!(document_path(refused), Err(Error::Refused(_))),
            "{refused}"
        );
    }
    for bad in ["", "/etc/passwd", "a/../b", "a\\b", "./a"] {
        assert!(
            matches!(document_path(bad), Err(Error::Invalid(_))),
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
