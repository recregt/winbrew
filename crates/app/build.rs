use chrono::{DateTime, Utc};
use std::process::Command;

const SOURCE_DATE_EPOCH: &str = "SOURCE_DATE_EPOCH";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed={SOURCE_DATE_EPOCH}");

    if let Some(git_dir) = git_dir() {
        println!("cargo::rerun-if-changed={git_dir}/HEAD");

        if let Some(reference) = git_head_reference(&git_dir) {
            println!("cargo::rerun-if-changed={git_dir}/{reference}");
        }
    }

    let git_hash = git_hash();
    println!("cargo::rustc-env=WINBREW_GIT_HASH={git_hash}");

    let build_date = build_date();
    println!("cargo::rustc-env=WINBREW_BUILD_DATE={build_date}");
}

fn build_date() -> String {
    std::env::var(SOURCE_DATE_EPOCH)
        .ok()
        .and_then(|ts| ts.parse::<i64>().ok())
        .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
        .or_else(|| git_commit_timestamp().and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0)))
        .unwrap_or_else(|| {
            DateTime::<Utc>::from_timestamp(0, 0).expect("unix epoch should be valid")
        })
        .to_rfc3339()
}

fn git_dir() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .map(|path| path.trim().to_owned())
}

fn git_head_reference(git_dir: &str) -> Option<String> {
    let head = std::fs::read_to_string(format!("{git_dir}/HEAD")).ok()?;
    head.trim().strip_prefix("ref: ").map(str::to_owned)
}

fn git_hash() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |hash| hash.trim().to_owned())
}

fn git_commit_timestamp() -> Option<i64> {
    let output = Command::new("git")
        .args(["show", "-s", "--format=%ct", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
}
