//! The files `--*-file` options name (`--description-file`, `--json-file`, `--body-file`, `--summary-file`,
//! `--meta-file`; ATM-662): read through one door. Agents pass them, often from a scratch folder outside the project
//! (the skills say so), so any folder is fine; but a key or environment file, a file in a credentials folder, anything
//! but a regular file (`/dev/zero`, a FIFO) and anything over 1 MiB are refused, so a pre-approved command can't put a
//! key into the task files that `atm vcs commit` then commits.

use std::io::Read as _;
use std::path::{Component, Path};

use atm_common::secrets::{SECRET_FOLDERS, env_file, key_file};
use atm_common::{Error, Result};

/// The largest file an option reads.
pub const MAX_OPTION_FILE: u64 = 1024 * 1024;

/// `--<option> <file>`'s text, `file` relative to `cwd` or absolute.
///
/// # Errors
/// `Invalid` (exit code 2), one wording for every file that can't be used: missing or unreadable, a key or
/// environment file, one in a credentials folder (also through a symbolic link), not a regular file, over 1 MiB, not
/// UTF-8.
pub fn read_option_file(cwd: &Path, option: &str, file: &str) -> Result<String> {
    let refused = |why: String| Error::Invalid(format!("--{option} {file}: {why}"));
    // Where it really is: a symbolic link is judged by what it points to
    let path = std::fs::canonicalize(cwd.join(file))
        .map_err(|e| refused(format!("can't read it ({e})")))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if key_file(&name) || env_file(&name) {
        return Err(refused("atm doesn't read key or environment files".into()));
    }
    let in_secret_folder = path.components().any(|c| match c {
        Component::Normal(part) => SECRET_FOLDERS.contains(&part.to_string_lossy().as_ref()),
        _ => false,
    });
    if in_secret_folder {
        return Err(refused(format!(
            "atm doesn't read files in a credentials folder ({})",
            SECRET_FOLDERS.join(", ")
        )));
    }
    let meta = std::fs::metadata(&path).map_err(|e| refused(format!("can't read it ({e})")))?;
    if !meta.is_file() {
        return Err(refused("not a regular file".into()));
    }
    if meta.len() > MAX_OPTION_FILE {
        return Err(refused("larger than 1 MiB".into()));
    }
    // Read no more than the limit, should it grow meanwhile
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|f| f.take(MAX_OPTION_FILE + 1).read_to_end(&mut bytes))
        .map_err(|e| refused(format!("can't read it ({e})")))?;
    if bytes.len() as u64 > MAX_OPTION_FILE {
        return Err(refused("larger than 1 MiB".into()));
    }
    String::from_utf8(bytes).map_err(|_| refused("not UTF-8 text".into()))
}

#[cfg(test)]
mod tests {
    use super::{MAX_OPTION_FILE, read_option_file};
    use atm_common::Error;

    struct Dir(std::path::PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn dir(name: &str) -> Dir {
        let path =
            std::env::temp_dir().join(format!("atm-option-file-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Dir(path)
    }
    fn refusal(result: atm_common::Result<String>) -> String {
        match result {
            Err(Error::Invalid(message)) => message,
            other => panic!("not refused as a usage error: {other:?}"),
        }
    }

    #[test]
    fn a_text_file_anywhere_is_read_relative_or_absolute() {
        let d = dir("ok");
        std::fs::write(d.0.join("summary.md"), "Why and what.\n").unwrap();
        assert_eq!(
            read_option_file(&d.0, "summary-file", "summary.md").unwrap(),
            "Why and what.\n"
        );
        let absolute = d.0.join("summary.md").to_string_lossy().into_owned();
        assert_eq!(
            read_option_file(Path::new("/"), "summary-file", &absolute).unwrap(),
            "Why and what.\n"
        );
    }
    use std::path::Path;

    #[test]
    fn key_and_environment_files_and_credentials_folders_are_refused() {
        let d = dir("keys");
        for name in ["id_ed25519", "server.pem", ".env", ".env.local"] {
            std::fs::write(d.0.join(name), "secret").unwrap();
            assert!(
                refusal(read_option_file(&d.0, "description-file", name))
                    .contains("key or environment"),
                "{name}"
            );
        }
        std::fs::create_dir_all(d.0.join(".ssh")).unwrap();
        std::fs::write(d.0.join(".ssh/config"), "Host x").unwrap();
        assert!(
            refusal(read_option_file(&d.0, "description-file", ".ssh/config"))
                .contains("credentials folder")
        );
        // An example environment file is shared on purpose
        std::fs::write(d.0.join(".env.example"), "KEY=").unwrap();
        assert_eq!(
            read_option_file(&d.0, "description-file", ".env.example").unwrap(),
            "KEY="
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_is_judged_by_what_it_points_to() {
        let d = dir("link");
        std::fs::create_dir_all(d.0.join(".ssh")).unwrap();
        std::fs::write(d.0.join(".ssh/id_rsa"), "secret").unwrap();
        std::os::unix::fs::symlink(d.0.join(".ssh/id_rsa"), d.0.join("notes.md")).unwrap();
        assert!(
            refusal(read_option_file(&d.0, "body-file", "notes.md")).contains("key or environment")
        );
    }

    #[test]
    fn a_missing_file_a_folder_a_device_too_large_or_not_utf8_is_refused() {
        let d = dir("bad");
        assert!(
            refusal(read_option_file(&d.0, "body-file", "gone.md"))
                .starts_with("--body-file gone.md: can't read it")
        );
        std::fs::create_dir_all(d.0.join("folder")).unwrap();
        assert!(
            refusal(read_option_file(&d.0, "body-file", "folder")).contains("not a regular file")
        );
        #[cfg(unix)]
        assert!(
            refusal(read_option_file(&d.0, "body-file", "/dev/zero"))
                .contains("not a regular file")
        );
        let big = usize::try_from(MAX_OPTION_FILE).unwrap() + 1;
        std::fs::write(d.0.join("big.md"), vec![b'x'; big]).unwrap();
        assert!(
            refusal(read_option_file(&d.0, "body-file", "big.md")).contains("larger than 1 MiB")
        );
        std::fs::write(d.0.join("max.md"), vec![b'x'; big - 1]).unwrap();
        assert!(
            read_option_file(&d.0, "body-file", "max.md").is_ok(),
            "1 MiB exactly is read"
        );
        std::fs::write(d.0.join("latin1.md"), [0xff, 0xfe]).unwrap();
        assert!(refusal(read_option_file(&d.0, "body-file", "latin1.md")).contains("not UTF-8"));
    }
}
