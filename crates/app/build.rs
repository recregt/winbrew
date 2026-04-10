use std::process::Command;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    if let Some(git_dir) = git_dir() {
        println!("cargo::rerun-if-changed={git_dir}/HEAD");

        if let Some(reference) = git_head_reference(&git_dir) {
            println!("cargo::rerun-if-changed={git_dir}/{reference}");
        }
    }

    let git_hash = git_hash();
    println!("cargo::rustc-env=WINBREW_GIT_HASH={git_hash}");
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
