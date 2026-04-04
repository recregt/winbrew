use chrono::{DateTime, Utc};
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

    let build_date = build_date();
    println!("cargo::rustc-env=WINBREW_BUILD_DATE={build_date}");

    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set("ProductName", "winbrew");
        res.set("FileDescription", "The Windows Package Manager");
        res.set("LegalCopyright", "Copyright © 2026 Recregt");
        res.set_manifest(
            r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <ws2:longPathAware xmlns:ws2="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</ws2:longPathAware>
      <ws3:activeCodePage xmlns:ws3="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</ws3:activeCodePage>
    </windowsSettings>
  </application>
</assembly>
"#,
        );

        if let Err(err) = res.compile() {
            println!("cargo::error=failed to compile Windows resources: {err}");
            std::process::exit(1);
        }
    }
}

fn build_date() -> String {
    std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|ts| ts.parse::<i64>().ok())
        .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
        .unwrap_or_else(Utc::now)
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
        .map(|hash| hash.trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}
