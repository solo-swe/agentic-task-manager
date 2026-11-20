//! atm's errors (docs/layered-architecture.md §4.2, §5), as the Python atm's `common/errors.py` has them: by what went
//! wrong, each with the message for the person or agent who asked and the CLI's exit code ([`exit`]).

/// The CLI's exit codes, the one table (ATM-664): agents act on them (`atm docs commands`). Each error kind maps to
/// one in [`Error::code`]; the REST API maps the same kinds to HTTP statuses.
pub mod exit {
    /// Done.
    pub const OK: u8 = 0;
    /// Something atm can't do: no such thing, a refusal, a store error.
    pub const ERROR: u8 = 1;
    /// The request itself is wrong: a usage error, an unknown value, a malformed ID ([`super::Error::Invalid`]).
    pub const USAGE: u8 = 2;
    /// Nothing to hand out: `atm loop next` with nothing ready.
    pub const NOTHING: u8 = 3;
    /// A wait whose time was up first (`atm review wait`, `atm review watch --once`). The same value as
    /// [`NOTHING`], as agents know it (docs/agent/reviews.md).
    pub const TIME_UP: u8 = 3;
    /// Another holds it: a task claimed by another agent, a lease run out ([`super::Error::Conflict`]).
    pub const CONFLICT: u8 = 4;
}

/// Something atm can't do. Each front end says it its own way: the CLI as `atm: <message>` and [`Error::code`], the
/// REST API as an HTTP status.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// No such task, sprint, release, component or project (exit code 1, HTTP 404).
    #[error("{0}")]
    NotFound(String),
    /// The request itself is wrong: an unknown option value, a malformed ID, a note too long (exit code 2, like a
    /// usage error; HTTP 400).
    #[error("{0}")]
    Invalid(String),
    /// The thing exists but atm won't hand it out: a project file it doesn't show (hidden, ignored, a key file)
    /// (exit code 1, HTTP 403).
    #[error("{0}")]
    Refused(String),
    /// Larger than atm hands out (exit code 1, HTTP 413).
    #[error("{0}")]
    TooLarge(String),
    /// Another holds it: a task claimed by another agent, or one's own lease run out (exit code 4, HTTP 409;
    /// ATM-664).
    #[error("{0}")]
    Conflict(String),
    /// Anything else atm refuses or can't do, with its exit code (1 unless said otherwise; HTTP 500).
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
            code: exit::ERROR,
        }
    }

    /// The CLI's exit code for this error.
    #[must_use]
    pub fn code(&self) -> u8 {
        match self {
            Self::NotFound(_) | Self::Refused(_) | Self::TooLarge(_) => exit::ERROR,
            Self::Invalid(_) => exit::USAGE,
            Self::Conflict(_) => exit::CONFLICT,
            Self::Failed { code, .. } => *code,
        }
    }
}

/// What atm's fallible functions return.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::{Error, exit};

    #[test]
    fn each_kind_has_its_exit_code() {
        assert_eq!(Error::NotFound("no task T-1".into()).code(), exit::ERROR);
        assert_eq!(Error::Refused("x".into()).code(), exit::ERROR);
        assert_eq!(Error::TooLarge("x".into()).code(), exit::ERROR);
        assert_eq!(Error::Invalid("bad".into()).code(), exit::USAGE);
        assert_eq!(Error::Conflict("claimed".into()).code(), exit::CONFLICT);
        assert_eq!(Error::failed("x").code(), exit::ERROR);
        assert_eq!(
            Error::Failed {
                message: "x".into(),
                code: 5
            }
            .code(),
            5
        );
        assert_eq!(
            (
                exit::OK,
                exit::ERROR,
                exit::USAGE,
                exit::NOTHING,
                exit::CONFLICT
            ),
            (0, 1, 2, 3, 4)
        );
        assert_eq!(exit::TIME_UP, 3, "as agents know it");
    }

    #[test]
    fn the_message_is_the_error_text() {
        assert_eq!(
            Error::NotFound("no task T-1".into()).to_string(),
            "no task T-1"
        );
        assert_eq!(Error::Conflict("claimed".into()).to_string(), "claimed");
    }
}
