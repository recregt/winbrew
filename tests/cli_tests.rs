use clap::Parser;
use winbrew::cli::{Cli, Command, ConfigCommand};

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
fn parse_info() {
    let cli = Cli::parse_from(["brew", "info"]);

    assert_eq!(cli.command, Command::Info);
}

#[test]
fn parse_doctor() {
    let cli = Cli::parse_from(["brew", "doctor"]);

    assert_eq!(cli.command, Command::Doctor);
}

#[test]
fn parse_install_with_single_word_query() {
    let cli = Cli::parse_from(["brew", "install", "ripgrep"]);

    assert_eq!(
        cli.command,
        Command::Install {
            query: vec!["ripgrep".to_string()],
            version: None,
            select: None,
        }
    );
}

#[test]
fn parse_install_with_version_flag() {
    let cli = Cli::parse_from([
        "brew",
        "install",
        "Microsoft.WindowsTerminal",
        "--version",
        "1.9.1942.0",
    ]);

    assert_eq!(
        cli.command,
        Command::Install {
            query: vec!["Microsoft.WindowsTerminal".to_string()],
            version: Some("1.9.1942.0".to_string()),
            select: None,
        }
    );
}

#[test]
fn parse_install_with_select_flag() {
    let cli = Cli::parse_from(["brew", "install", "windows", "terminal", "--select", "1"]);

    assert_eq!(
        cli.command,
        Command::Install {
            query: vec!["windows".to_string(), "terminal".to_string()],
            version: None,
            select: Some(1),
        }
    );
}

#[test]
fn parse_install_with_multi_word_query() {
    let cli = Cli::parse_from(["brew", "install", "windows", "terminal"]);

    assert_eq!(
        cli.command,
        Command::Install {
            query: vec!["windows".to_string(), "terminal".to_string()],
            version: None,
            select: None,
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
