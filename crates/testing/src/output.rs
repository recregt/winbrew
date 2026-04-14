use anyhow::ensure;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("testing crate should live under crates/")
        .to_path_buf()
}

pub fn run_winbrew(root: &Path, args: &[&str]) -> Output {
    Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(repo_root().join("Cargo.toml"))
        .arg("-p")
        .arg("winbrew-bin")
        .arg("--bin")
        .arg("winbrew")
        .arg("--")
        .args(args)
        .env("WINBREW_PATHS_ROOT", root)
        .env("NO_COLOR", "1")
        .current_dir(repo_root())
        .output()
        .expect("failed to run winbrew binary")
}

pub fn output_text(output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}

pub fn assert_success(output: &Output, context: &str) -> anyhow::Result<()> {
    ensure!(
        output.status.success(),
        "{context} failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(())
}

pub fn assert_output_contains(output: &Output, expected: &str) -> anyhow::Result<()> {
    let text = output_text(output);
    ensure!(
        text.contains(expected),
        "Expected output to contain: {expected}\nActual output:\n{text}"
    );
    Ok(())
}

pub fn assert_output_contains_all(output: &Output, expected: &[&str]) -> anyhow::Result<()> {
    let text = output_text(output);
    for pattern in expected {
        ensure!(
            text.contains(pattern),
            "Expected output to contain: {pattern}\nActual output:\n{text}"
        );
    }
    Ok(())
}
