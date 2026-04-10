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
    CommandError::reported(message).into()
}

pub fn reported_with_hint(message: impl Into<String>, hint: impl Into<String>) -> anyhow::Error {
    CommandError::reported(message).with_hint(hint).into()
}

pub fn cancelled() -> anyhow::Error {
    CommandError::cancelled().into()
}

pub fn fatal(message: impl Into<String>) -> anyhow::Error {
    CommandError::fatal(message).into()
}

pub fn is_handled(err: &anyhow::Error) -> bool {
    err.downcast_ref::<CommandError>().is_some()
}

impl CommandError {
    pub fn reported(message: impl Into<String>) -> Self {
        Self::Reported {
            message: message.into(),
            exit_code: 1,
            hint: None,
        }
    }

    pub fn cancelled() -> Self {
        Self::Cancelled
    }

    pub fn fatal(message: impl Into<String>) -> Self {
        Self::Fatal(message.into())
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        if let Self::Reported {
            hint: current_hint, ..
        } = &mut self
        {
            *current_hint = Some(hint.into());
        }

        self
    }

    pub fn with_exit_code(mut self, exit_code: i32) -> Self {
        if let Self::Reported {
            exit_code: current_exit_code,
            ..
        } = &mut self
        {
            *current_exit_code = exit_code;
        }

        self
    }

    pub fn as_reported(&self) -> Option<(&str, i32, Option<&str>)> {
        match self {
            Self::Reported {
                message,
                exit_code,
                hint,
            } => Some((message.as_str(), *exit_code, hint.as_deref())),
            _ => None,
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Reported { exit_code, .. } => *exit_code,
            Self::Cancelled => 130,
            Self::Fatal(_) => 1,
        }
    }

    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Fatal(_))
    }

    pub fn hint(&self) -> Option<&str> {
        self.as_reported().and_then(|(_, _, hint)| hint)
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
    fn reported_builder_can_customize_hint_and_exit_code() {
        let err = CommandError::reported("already shown")
            .with_hint("try again later")
            .with_exit_code(2);

        assert_eq!(
            err.as_reported(),
            Some(("already shown", 2, Some("try again later")))
        );
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
        assert!(cmd_err.is_fatal());
        assert_eq!(cmd_err.exit_code(), 1);
    }
}
