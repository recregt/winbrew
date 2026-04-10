use std::process::ExitCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("{message}")]
    Reported {
        message: String,
        exit_code: i32,
        hint: Option<String>,
        #[source]
        source: Option<anyhow::Error>,
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

pub fn reported_with_source(
    message: impl Into<String>,
    source: Option<anyhow::Error>,
) -> anyhow::Error {
    CommandError::reported(message).with_source(source).into()
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
            source: None,
        }
    }

    pub fn cancelled() -> Self {
        Self::Cancelled
    }

    pub fn fatal(message: impl Into<String>) -> Self {
        Self::Fatal(message.into())
    }

    #[must_use = "builder method returns a modified error and should be used"]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        if let Self::Reported {
            hint: current_hint, ..
        } = &mut self
        {
            *current_hint = Some(hint.into());
        }

        self
    }

    #[must_use = "builder method returns a modified error and should be used"]
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

    #[must_use = "builder method returns a modified error and should be used"]
    pub fn with_source(mut self, source: Option<anyhow::Error>) -> Self {
        if let Self::Reported {
            source: current_source,
            ..
        } = &mut self
        {
            *current_source = source;
        }

        self
    }

    pub fn as_reported(&self) -> Option<(&str, i32, Option<&str>)> {
        match self {
            Self::Reported {
                message,
                exit_code,
                hint,
                ..
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

impl PartialEq for CommandError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Reported {
                    message: left_message,
                    exit_code: left_exit_code,
                    hint: left_hint,
                    ..
                },
                Self::Reported {
                    message: right_message,
                    exit_code: right_exit_code,
                    hint: right_hint,
                    ..
                },
            ) => {
                left_message == right_message
                    && left_exit_code == right_exit_code
                    && left_hint == right_hint
            }
            (Self::Cancelled, Self::Cancelled) => true,
            (Self::Fatal(left), Self::Fatal(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for CommandError {}

impl From<&CommandError> for ExitCode {
    fn from(err: &CommandError) -> Self {
        ExitCode::from(err.exit_code().clamp(0, 255) as u8)
    }
}

impl From<CommandError> for ExitCode {
    fn from(err: CommandError) -> Self {
        ExitCode::from(&err)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandError, cancelled, fatal, is_handled, reported, reported_with_hint,
        reported_with_source,
    };
    use std::process::ExitCode;

    #[test]
    fn reported_errors_are_handled_and_default_to_exit_one() {
        let err = reported("already shown");

        let cmd_err = err.downcast_ref::<CommandError>().expect("command error");
        assert_eq!(cmd_err, &CommandError::reported("already shown"));
        assert_eq!(cmd_err.exit_code(), 1);
        assert!(is_handled(&err));
    }

    #[test]
    fn reported_errors_can_carry_hints() {
        let err = reported_with_hint("already shown", "try again later");

        let cmd_err = err.downcast_ref::<CommandError>().expect("command error");
        assert_eq!(
            cmd_err,
            &CommandError::reported("already shown").with_hint("try again later")
        );
        assert_eq!(cmd_err.hint(), Some("try again later"));
    }

    #[test]
    fn reported_errors_can_carry_sources() {
        let source = std::io::Error::other("disk full");
        let err = reported_with_source("already shown", Some(source.into()));

        let cmd_err = err.downcast_ref::<CommandError>().expect("command error");
        assert!(std::error::Error::source(cmd_err).is_some());
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
        assert_eq!(cmd_err, &CommandError::Cancelled);
        assert_eq!(cmd_err.exit_code(), 130);
    }

    #[test]
    fn fatal_errors_exit_with_one() {
        let err = fatal("boom");

        let cmd_err = err.downcast_ref::<CommandError>().expect("command error");
        assert_eq!(cmd_err, &CommandError::Fatal("boom".to_string()));
        assert!(cmd_err.is_fatal());
        assert_eq!(cmd_err.exit_code(), 1);
    }

    #[test]
    fn command_error_converts_to_exit_code() {
        let reported = CommandError::reported("already shown").with_exit_code(2);
        let cancelled = CommandError::Cancelled;
        let fatal = CommandError::Fatal("boom".to_string());

        assert_eq!(ExitCode::from(&reported), ExitCode::from(2));
        assert_eq!(ExitCode::from(&cancelled), ExitCode::from(130));
        assert_eq!(ExitCode::from(&fatal), ExitCode::from(1));
    }
}
