use std::env;
use std::ffi::OsString;

use clap::Parser;
use winbrew_cli::cli::{Cli, Command, ConfigCommand};
use winbrew_cli::run_app;

#[test]
fn parse_list() {
    let cli = Cli::parse_from(["brew", "list"]);

    assert_eq!(cli.command, Command::List { query: vec![] });
}

#[test]
fn parse_list_with_query() {
    let cli = Cli::parse_from(["brew", "list", "python"]);

    assert_eq!(
        cli.command,
        Command::List {
            query: vec!["python".to_string()],
        }
    );
}

#[test]
fn parse_search_with_query() {
    let cli = Cli::parse_from(["brew", "search", "python"]);

    assert_eq!(
        cli.command,
        Command::Search {
            query: vec!["python".to_string()],
        }
    );
}

#[test]
fn parse_info() {
    let cli = Cli::parse_from(["brew", "info"]);

    assert_eq!(cli.command, Command::Info);
}

#[test]
fn parse_version() {
    let cli = Cli::parse_from(["brew", "version"]);

    assert_eq!(cli.command, Command::Version);
}

#[test]
fn parse_update() {
    let cli = Cli::parse_from(["brew", "update"]);

    assert_eq!(cli.command, Command::Update);
}

#[test]
fn parse_install_with_ignore_checksum_security() {
    let cli = Cli::parse_from(["brew", "install", "gzip", "--ignore-checksum-security"]);

    assert_eq!(
        cli.command,
        Command::Install {
            query: vec!["gzip".to_string()],
            ignore_checksum_security: true,
        }
    );
}

#[test]
fn parse_install_without_ignore_checksum_security() {
    let cli = Cli::parse_from(["brew", "install", "gzip"]);

    assert_eq!(
        cli.command,
        Command::Install {
            query: vec!["gzip".to_string()],
            ignore_checksum_security: false,
        }
    );
}

#[test]
fn parse_remove_with_yes() {
    let cli = Cli::parse_from(["brew", "remove", "ripgrep", "--yes"]);

    assert_eq!(
        cli.command,
        Command::Remove {
            name: "ripgrep".to_string(),
            yes: true,
            force: false,
        }
    );
}

#[test]
fn parse_remove_with_force() {
    let cli = Cli::parse_from(["brew", "remove", "ripgrep", "--force"]);

    assert_eq!(
        cli.command,
        Command::Remove {
            name: "ripgrep".to_string(),
            yes: false,
            force: true,
        }
    );
}

#[test]
fn parse_repair_with_yes() {
    let cli = Cli::parse_from(["brew", "repair", "--yes"]);

    assert_eq!(cli.command, Command::Repair { yes: true });
}

#[test]
fn parse_config_list() {
    let cli = Cli::parse_from(["brew", "config", "list"]);

    assert_eq!(
        cli.command,
        Command::Config {
            command: ConfigCommand::List,
        }
    );
}

#[test]
fn parse_config_set_core_log_level() {
    let cli = Cli::parse_from(["brew", "config", "set", "core.log_level", "debug"]);

    assert_eq!(
        cli.command,
        Command::Config {
            command: ConfigCommand::Set {
                key: "core.log_level".to_string(),
                value: Some("debug".to_string()),
            },
        }
    );
}

#[test]
fn parse_config_set_without_value() {
    let cli = Cli::parse_from(["brew", "config", "set", "core.log_level"]);

    assert_eq!(
        cli.command,
        Command::Config {
            command: ConfigCommand::Set {
                key: "core.log_level".to_string(),
                value: None,
            },
        }
    );
}

#[test]
fn parse_config_unset_core_log_level() {
    let cli = Cli::parse_from(["brew", "config", "unset", "core.log_level"]);

    assert_eq!(
        cli.command,
        Command::Config {
            command: ConfigCommand::Unset {
                key: "core.log_level".to_string(),
            },
        }
    );
}

#[test]
fn parse_config_get_core_log_level() {
    let cli = Cli::parse_from(["brew", "config", "get", "core.log_level"]);

    assert_eq!(
        cli.command,
        Command::Config {
            command: ConfigCommand::Get {
                key: "core.log_level".to_string(),
            },
        }
    );
}

#[test]
fn parse_doctor() {
    let cli = Cli::parse_from(["brew", "doctor"]);

    assert_eq!(
        cli.command,
        Command::Doctor {
            json: false,
            warn_as_error: false,
        }
    );
}

#[test]
fn parse_doctor_json() {
    let cli = Cli::parse_from(["brew", "doctor", "--json"]);

    assert_eq!(
        cli.command,
        Command::Doctor {
            json: true,
            warn_as_error: false,
        }
    );
}

#[test]
fn parse_doctor_warn_as_error() {
    let cli = Cli::parse_from(["brew", "doctor", "--warn-as-error"]);

    assert_eq!(
        cli.command,
        Command::Doctor {
            json: false,
            warn_as_error: true,
        }
    );
}

#[test]
fn run_app_version_smoke_test() {
    let temp_root = tempfile::tempdir().expect("temp root");
    let _env_guard = EnvOverrideGuard::set("WINBREW_PATHS_ROOT", temp_root.path().as_os_str());

    run_app(Command::Version, 0).expect("run_app should succeed");
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
