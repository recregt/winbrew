use std::env;
use std::ffi::OsString;

use clap::{Parser, error::ErrorKind};
use winbrew_cli::cli::{Cli, Command, ConfigCommand};
use winbrew_cli::run_app;

type ParseCase = (Vec<&'static str>, fn() -> Command);

macro_rules! assert_parse {
    ($args:expr, $expected:expr) => {{
        let cli = Cli::parse_from($args);
        assert_eq!(cli.command, $expected);
    }};
}

fn assert_command_case<F>(args: Vec<&str>, expected: F)
where
    F: FnOnce() -> Command,
{
    let cli = Cli::parse_from(args);
    assert_eq!(cli.command, expected());
}

fn assert_parse_error(args: &[&str], expected_kind: ErrorKind, expected_fragments: &[&str]) {
    let err = match Cli::try_parse_from(args) {
        Ok(_) => panic!("parse unexpectedly succeeded for {args:?}"),
        Err(err) => err,
    };
    assert_eq!(
        err.kind(),
        expected_kind,
        "unexpected error kind for {args:?}"
    );

    let text = err.to_string();
    for fragment in expected_fragments {
        assert!(
            text.contains(fragment),
            "Expected parse error for {args:?} to contain `{fragment}`\nActual output:\n{text}"
        );
    }
}

struct EnvOverrideGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvOverrideGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let previous = env::var_os(key);
        unsafe {
            env::set_var(key, value);
        }

        Self { key, previous }
    }
}

impl Drop for EnvOverrideGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            unsafe {
                env::set_var(self.key, previous);
            }
        } else {
            unsafe {
                env::remove_var(self.key);
            }
        }
    }
}

mod list_tests {
    use super::*;

    #[test]
    fn parses_list_queries() {
        let cases: Vec<ParseCase> = vec![
            (vec!["brew", "list"], || Command::List { query: vec![] }),
            (vec!["brew", "list", "python"], || Command::List {
                query: vec!["python".to_string()],
            }),
            (vec!["brew", "list", "python", "node"], || Command::List {
                query: vec!["python".to_string(), "node".to_string()],
            }),
        ];

        for (args, expected) in cases {
            assert_command_case(args, expected);
        }
    }
}

mod search_tests {
    use super::*;

    #[test]
    fn parses_search_queries() {
        let cases: Vec<ParseCase> = vec![
            (vec!["brew", "search"], || Command::Search { query: vec![] }),
            (vec!["brew", "search", "python"], || Command::Search {
                query: vec!["python".to_string()],
            }),
            (vec!["brew", "search", "python", "node"], || {
                Command::Search {
                    query: vec!["python".to_string(), "node".to_string()],
                }
            }),
        ];

        for (args, expected) in cases {
            assert_command_case(args, expected);
        }
    }
}

mod install_tests {
    use super::*;

    #[test]
    fn parses_install_variants() {
        let cases: Vec<ParseCase> = vec![
            (vec!["brew", "install"], || Command::Install {
                query: vec![],
                ignore_checksum_security: false,
                plan: false,
            }),
            (vec!["brew", "install", "gzip"], || Command::Install {
                query: vec!["gzip".to_string()],
                ignore_checksum_security: false,
                plan: false,
            }),
            (
                vec!["brew", "install", "gzip", "--ignore-checksum-security"],
                || Command::Install {
                    query: vec!["gzip".to_string()],
                    ignore_checksum_security: true,
                    plan: false,
                },
            ),
            (vec!["brew", "install", "gzip", "--plan"], || {
                Command::Install {
                    query: vec!["gzip".to_string()],
                    ignore_checksum_security: false,
                    plan: true,
                }
            }),
            (vec!["brew", "install", "gzip", "curl"], || {
                Command::Install {
                    query: vec!["gzip".to_string(), "curl".to_string()],
                    ignore_checksum_security: false,
                    plan: false,
                }
            }),
        ];

        for (args, expected) in cases {
            assert_command_case(args, expected);
        }
    }
}

mod remove_tests {
    use super::*;

    #[test]
    fn parses_remove_variants() {
        let cases: Vec<ParseCase> = vec![
            (vec!["brew", "remove", "ripgrep"], || Command::Remove {
                name: "ripgrep".to_string(),
                yes: false,
                force: false,
            }),
            (vec!["brew", "remove", "ripgrep", "--yes"], || {
                Command::Remove {
                    name: "ripgrep".to_string(),
                    yes: true,
                    force: false,
                }
            }),
            (vec!["brew", "remove", "ripgrep", "--force"], || {
                Command::Remove {
                    name: "ripgrep".to_string(),
                    yes: false,
                    force: true,
                }
            }),
            (
                vec!["brew", "remove", "package", "--yes", "--force"],
                || Command::Remove {
                    name: "package".to_string(),
                    yes: true,
                    force: true,
                },
            ),
        ];

        for (args, expected) in cases {
            assert_command_case(args, expected);
        }
    }
}

mod repair_tests {
    use super::*;

    #[test]
    fn parses_repair_variants() {
        let cases: Vec<ParseCase> = vec![
            (vec!["brew", "repair"], || Command::Repair { yes: false }),
            (vec!["brew", "repair", "--yes"], || Command::Repair {
                yes: true,
            }),
        ];

        for (args, expected) in cases {
            assert_command_case(args, expected);
        }
    }
}

mod config_tests {
    use super::*;

    #[test]
    fn parses_config_variants() {
        let cases: Vec<ParseCase> = vec![
            (vec!["brew", "config", "list"], || Command::Config {
                command: ConfigCommand::List,
            }),
            (vec!["brew", "config", "get", "core.log_level"], || {
                Command::Config {
                    command: ConfigCommand::Get {
                        key: "core.log_level".to_string(),
                    },
                }
            }),
            (
                vec!["brew", "config", "set", "core.log_level", "debug"],
                || Command::Config {
                    command: ConfigCommand::Set {
                        key: "core.log_level".to_string(),
                        value: Some("debug".to_string()),
                    },
                },
            ),
            (vec!["brew", "config", "set", "core.log_level"], || {
                Command::Config {
                    command: ConfigCommand::Set {
                        key: "core.log_level".to_string(),
                        value: None,
                    },
                }
            }),
            (vec!["brew", "config", "unset", "core.log_level"], || {
                Command::Config {
                    command: ConfigCommand::Unset {
                        key: "core.log_level".to_string(),
                    },
                }
            }),
        ];

        for (args, expected) in cases {
            assert_command_case(args, expected);
        }
    }
}

mod doctor_tests {
    use super::*;

    #[test]
    fn parses_doctor_variants() {
        let cases: Vec<ParseCase> = vec![
            (vec!["brew", "doctor"], || Command::Doctor {
                json: false,
                warn_as_error: false,
            }),
            (vec!["brew", "doctor", "--json"], || Command::Doctor {
                json: true,
                warn_as_error: false,
            }),
            (vec!["brew", "doctor", "--warn-as-error"], || {
                Command::Doctor {
                    json: false,
                    warn_as_error: true,
                }
            }),
            (vec!["brew", "doctor", "--json", "--warn-as-error"], || {
                Command::Doctor {
                    json: true,
                    warn_as_error: true,
                }
            }),
        ];

        for (args, expected) in cases {
            assert_command_case(args, expected);
        }
    }

    #[test]
    fn parses_verbose_doctor() {
        let cli = Cli::parse_from(["brew", "-vv", "doctor"]);

        assert_eq!(cli.verbose, 2);
        assert_eq!(
            cli.command,
            Command::Doctor {
                json: false,
                warn_as_error: false,
            }
        );
    }
}

mod simple_command_tests {
    use super::*;

    #[test]
    fn parses_info() {
        assert_parse!(["brew", "info"], Command::Info);
    }

    #[test]
    fn parses_version() {
        assert_parse!(["brew", "version"], Command::Version);
    }

    #[test]
    fn parses_update() {
        assert_parse!(["brew", "update"], Command::Update);
    }
}

/// Parser failure cases for invalid CLI input and missing arguments.
mod negative_tests {
    use super::*;

    #[test]
    fn rejects_unknown_command() {
        assert_parse_error(
            ["brew", "nonexistent_command"].as_slice(),
            ErrorKind::InvalidSubcommand,
            &["nonexistent_command"],
        );
    }

    #[test]
    fn rejects_invalid_flag() {
        assert_parse_error(
            ["brew", "install", "gzip", "--invalid-flag"].as_slice(),
            ErrorKind::UnknownArgument,
            &["--invalid-flag"],
        );
    }

    #[test]
    fn rejects_missing_remove_name() {
        assert_parse_error(
            ["brew", "remove"].as_slice(),
            ErrorKind::MissingRequiredArgument,
            &["remove"],
        );
    }

    #[test]
    fn shows_help_for_config_without_action() {
        assert_parse_error(
            ["brew", "config"].as_slice(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
            &["Usage:", "Commands:", "list", "set", "get", "unset"],
        );
    }

    #[test]
    fn shows_help_when_no_command_is_provided() {
        assert_parse_error(
            ["brew"].as_slice(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
            &["Usage:", "Commands:"],
        );
    }
}

/// Help text regression checks for the parser surface.
mod help_validation {
    use super::*;

    #[test]
    fn main_help_contains_expected_sections() {
        assert_parse_error(
            ["brew", "--help"].as_slice(),
            ErrorKind::DisplayHelp,
            &["Usage:", "Commands:", "Options:"],
        );
    }

    #[test]
    fn install_help_documents_checksum_flag() {
        assert_parse_error(
            ["brew", "install", "--help"].as_slice(),
            ErrorKind::DisplayHelp,
            &["--ignore-checksum-security", "Install", "Usage:"],
        );
    }

    #[test]
    fn config_help_lists_all_actions() {
        assert_parse_error(
            ["brew", "config", "--help"].as_slice(),
            ErrorKind::DisplayHelp,
            &["list", "get", "set", "unset"],
        );
    }
}

mod integration {
    use super::*;

    #[test]
    fn run_app_version_smoke_test() {
        let temp_root = tempfile::tempdir().expect("temp root");
        let _env_guard = EnvOverrideGuard::set("WINBREW_PATHS_ROOT", temp_root.path().as_os_str());

        run_app(Command::Version, 0).expect("run_app should succeed");
    }
}
