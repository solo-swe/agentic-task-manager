//! A revision as atm gives it to git (ATM-659): checked when it is made, so no string that git could read as an
//! option (`--output=…`, `--index-output=…`) reaches it, whoever wrote it (a review's round files are committed and
//! merged, so anyone who can push writes them).

use std::fmt;

use atm_common::{Error, Result};

/// A revision to give git: a full object ID (a commit's, a tree's, a blob's), or a name (`HEAD`, `main`, `HEAD~1`,
/// a short SHA) that doesn't start with `-` and has no control character or space. Every method of this crate that
/// hands git a revision takes one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rev(String);

/// Whether `text` is a full object ID: SHA-1's 40 hexadecimal digits, or SHA-256's 64.
#[must_use]
pub fn is_object_id(text: &str) -> bool {
    matches!(text.len(), 40 | 64) && text.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Rev {
    /// A revision by name or ID.
    ///
    /// # Errors
    /// `Invalid` for an empty one, one starting with `-` (git would read it as an option), or one with a control
    /// character (a NUL, a newline) or a space.
    pub fn new(name: &str) -> Result<Self> {
        if name.is_empty()
            || name.starts_with('-')
            || name.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(Error::Invalid(format!("{name:?}: not a revision")));
        }
        Ok(Self(name.to_string()))
    }

    /// A full object ID only (what atm records: a round's commits, a tree it rebuilt).
    ///
    /// # Errors
    /// `Invalid` for anything else.
    pub fn object_id(id: &str) -> Result<Self> {
        if !is_object_id(id) {
            return Err(Error::Invalid(format!("{id:?}: not a full object ID")));
        }
        Ok(Self(id.to_string()))
    }

    /// `HEAD`: the commit checked out.
    #[must_use]
    pub fn head() -> Self {
        Self("HEAD".into())
    }

    /// The revision as git reads it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Rev {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{Rev, is_object_id};

    const SHA1: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn a_name_is_taken_unless_git_could_read_it_as_an_option_or_it_has_a_control_character() {
        for ok in [
            "HEAD",
            "main",
            "HEAD~1",
            "dev/0.43.0",
            "abc1234",
            "v1.2.3^{commit}",
            SHA1,
        ] {
            assert_eq!(Rev::new(ok).unwrap().as_str(), ok);
        }
        for bad in [
            "",
            "-",
            "--index-output=x",
            "--output=/tmp/x",
            "-p",
            "HEAD\0",
            "HEAD\n--all",
            "main branch",
            "a\tb",
        ] {
            assert!(Rev::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn an_object_id_is_sha1s_or_sha256s_full_hex() {
        let sha256 = "a".repeat(64);
        for ok in [SHA1, sha256.as_str(), &SHA1.to_uppercase()] {
            assert!(is_object_id(ok), "{ok}");
            assert!(Rev::object_id(ok).is_ok(), "{ok}");
        }
        for bad in [
            "HEAD",
            "abc1234",
            "--index-output=x",
            &SHA1[..39],
            &format!("{SHA1}0"),
            &"g".repeat(40),
        ] {
            assert!(!is_object_id(bad), "{bad}");
            assert!(Rev::object_id(bad).is_err(), "{bad}");
        }
    }
}
