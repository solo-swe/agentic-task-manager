//! atm's errors (docs/layered-architecture.md §4.2, §5), as the Python atm's `common/errors.py` has them: by what went
//! wrong, each with the message for the person or agent who asked and the CLI's exit code.

/// Something atm can't do. Each front end says it its own way: the CLI as `atm: <message>` and [`Error::code`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// No such task, sprint, release, component or project (exit code 1).
    #[error("{0}")]
    NotFound(String),
    /// The request itself is wrong: an unknown option value, a malformed ID (exit code 2, like a usage error).
    #[error("{0}")]
    Invalid(String),
    /// The thing exists but atm won't hand it out: a project file it doesn't show (hidden, ignored, a key file)
    /// (exit code 1).
    #[error("{0}")]
    Refused(String),
    /// Larger than atm hands out (exit code 1).
    #[error("{0}")]
    TooLarge(String),
    /// Anything else atm refuses or can't do, with its exit code (1 unless said otherwise).
    #[error("{message}")]
    Failed {
        /// What went wrong, in words.
        message: String,
        /// The CLI's exit code.
        code: u8,
    },
}

impl Error {
    /// A failure with exit code 1, the most common kind.
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
            code: 1,
        }
    }

    /// The CLI's exit code for this error.
    #[must_use]
    pub fn code(&self) -> u8 {
        match self {
            Self::NotFound(_) | Self::Refused(_) | Self::TooLarge(_) => 1,
            Self::Invalid(_) => 2,
            Self::Failed { code, .. } => *code,
        }
    }
}

/// What atm's fallible functions return.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn each_kind_has_its_exit_code() {
        assert_eq!(Error::NotFound("no task T-1".into()).code(), 1);
        assert_eq!(Error::Invalid("bad".into()).code(), 2);
        assert_eq!(Error::failed("x").code(), 1);
        assert_eq!(
            Error::Failed {
                message: "x".into(),
                code: 4
            }
            .code(),
            4
        );
    }

    #[test]
    fn the_message_is_the_error_text() {
        assert_eq!(
            Error::NotFound("no task T-1".into()).to_string(),
            "no task T-1"
        );
    }
}
