use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("{message}")]
    Reported {
        message: String,
        exit_code: i32,
        hint: Option<String>,
    },

    #[error("operation cancelled")]
    Cancelled,

    #[error("{0}")]
    Fatal(String),
}

pub fn reported(message: impl Into<String>) -> anyhow::Error {
    CommandError::Reported {
        message: message.into(),
        exit_code: 1,
        hint: None,
    }
    .into()
}

pub fn reported_with_hint(message: impl Into<String>, hint: impl Into<String>) -> anyhow::Error {
    CommandError::Reported {
        message: message.into(),
        exit_code: 1,
        hint: Some(hint.into()),
    }
    .into()
}

pub fn cancelled() -> anyhow::Error {
    CommandError::Cancelled.into()
}

pub fn fatal(message: impl Into<String>) -> anyhow::Error {
    CommandError::Fatal(message.into()).into()
}

pub fn is_handled(err: &anyhow::Error) -> bool {
    err.downcast_ref::<CommandError>().is_some()
}

impl CommandError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Reported { exit_code, .. } => *exit_code,
            Self::Cancelled => 130,
            Self::Fatal(_) => 1,
        }
    }

    pub fn hint(&self) -> Option<&str> {
        match self {
            Self::Reported {
                hint: Some(hint), ..
            } => Some(hint.as_str()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandError, cancelled, fatal, is_handled, reported, reported_with_hint};

    #[test]
    fn reported_errors_are_handled_and_default_to_exit_one() {
        let err = reported("already shown");

        let cmd_err = err.downcast_ref::<CommandError>().expect("command error");
        assert!(matches!(cmd_err, CommandError::Reported { hint: None, .. }));
        assert_eq!(cmd_err.exit_code(), 1);
        assert!(is_handled(&err));
    }

    #[test]
    fn reported_errors_can_carry_hints() {
        let err = reported_with_hint("already shown", "try again later");

        let cmd_err = err.downcast_ref::<CommandError>().expect("command error");
        assert_eq!(cmd_err.hint(), Some("try again later"));
    }

    #[test]
    fn cancelled_errors_exit_with_130() {
        let err = cancelled();

        let cmd_err = err.downcast_ref::<CommandError>().expect("command error");
        assert!(matches!(cmd_err, CommandError::Cancelled));
        assert_eq!(cmd_err.exit_code(), 130);
    }

    #[test]
    fn fatal_errors_exit_with_one() {
        let err = fatal("boom");

        let cmd_err = err.downcast_ref::<CommandError>().expect("command error");
        assert!(matches!(cmd_err, CommandError::Fatal(_)));
        assert_eq!(cmd_err.exit_code(), 1);
    }
}
