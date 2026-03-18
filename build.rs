use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let git_hash = git_hash();
    println!("cargo:rustc-env=WINBREW_GIT_HASH={git_hash}");

    let build_date = chrono::Utc::now().to_rfc3339();
    println!("cargo:rustc-env=WINBREW_BUILD_DATE={build_date}");

    #[cfg(target_os = "windows")]
    {
        println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");

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
            panic!("failed to compile Windows resources: {err}");
        }
    }
}

fn git_hash() -> String {
    println!("cargo:rerun-if-changed=.git/HEAD");

    if let Ok(head) = std::fs::read_to_string(".git/HEAD")
        && let Some(reference) = head.trim().strip_prefix("ref: ")
    {
        println!("cargo:rerun-if-changed=.git/{reference}");
    }

    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}
