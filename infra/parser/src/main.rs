use std::env;
use std::io;
use std::path::PathBuf;
use std::process;

use winbrew_infra_parser::{RunConfig, run};

fn main() {
    let config = match parse_args() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            process::exit(2);
        }
    };

    if let Err(err) = run(io::stdin().lock(), config) {
        eprintln!("parser failed: {err}");
        process::exit(1);
    }
}

fn parse_args() -> Result<RunConfig, String> {
    let mut winget_jsonl_path: Option<PathBuf> = None;
    let mut output_db_path: Option<PathBuf> = None;
    let mut metadata_path: Option<PathBuf> = None;

    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        let arg_text = arg.to_string_lossy();
        match arg_text.as_ref() {
            "--winget-jsonl" | "--winget-db" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--winget-jsonl requires a value".to_string())?;
                winget_jsonl_path = Some(PathBuf::from(value));
            }
            "--out" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--out requires a value".to_string())?;
                output_db_path = Some(PathBuf::from(value));
            }
            "--metadata" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--metadata requires a value".to_string())?;
                metadata_path = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                return Err(help_text());
            }
            other => {
                return Err(format!("unknown argument: {other}\n{}", help_text()));
            }
        }
    }

    let winget_jsonl_path = winget_jsonl_path.ok_or_else(help_text_missing)?;
    let output_db_path = output_db_path.ok_or_else(help_text_missing)?;
    let mut config = RunConfig::new(winget_jsonl_path, output_db_path);
    if let Some(metadata_path) = metadata_path {
        config = config.with_metadata_path(metadata_path);
    }

    Ok(config)
}

fn help_text_missing() -> String {
    format!("missing required arguments\n{}", help_text())
}

fn help_text() -> String {
    [
        "Usage:",
        "  winbrew-infra-parser --winget-jsonl <path> --out <catalog.db> [--metadata <metadata.json>]",
    ]
    .join("\n")
}
