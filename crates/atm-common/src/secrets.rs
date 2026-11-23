//! Files that hold secrets, by their names: what atm never shows (the document viewer, atm-docs) nor reads into the
//! task files (`--*-file` options, ATM-662), whatever git or the path says.

/// Whether a file name is one that holds keys: `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.keystore`, `*.jks`, and SSH's
/// `id_rsa`, `id_dsa`, `id_ecdsa`, `id_ed25519` (with any extension), in any case.
#[must_use]
pub fn key_file(name: &str) -> bool {
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

/// Whether a file name is an environment file: `.env`, `.env.local`, … but not `.env.example`, `.env.sample` or
/// `.env.template` (shared on purpose, without values).
#[must_use]
pub fn env_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let example = [".example", ".sample", ".template"]
        .iter()
        .any(|e| lower.ends_with(e));
    (lower == ".env" || lower.starts_with(".env.")) && !example
}

/// The folders whose every file holds credentials: SSH's (`.ssh`), `GnuPG`'s (`.gnupg`), the AWS CLI's (`.aws`).
pub const SECRET_FOLDERS: [&str; 3] = [".ssh", ".gnupg", ".aws"];

#[cfg(test)]
mod tests {
    use super::{env_file, key_file};

    #[test]
    fn key_files_are_known_by_their_extension_or_ssh_name_in_any_case() {
        for name in [
            "a.pem",
            "SERVER.KEY",
            "c.p12",
            "d.pfx",
            "e.keystore",
            "f.jks",
            "id_rsa",
            "id_ed25519.pub",
            "ID_ECDSA",
        ] {
            assert!(key_file(name), "{name}");
        }
        for name in ["key.txt", "monkey", "id_rsa_notes.md", "keys.md", "pem"] {
            assert!(!key_file(name), "{name}");
        }
    }

    #[test]
    fn environment_files_but_not_their_examples() {
        for name in [".env", ".env.local", ".ENV.production"] {
            assert!(env_file(name), "{name}");
        }
        for name in [
            ".env.example",
            ".env.sample",
            ".env.template",
            "env",
            "a.env",
            ".environment",
        ] {
            assert!(!env_file(name), "{name}");
        }
    }
}
