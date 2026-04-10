fn main() {
    println!("cargo::rerun-if-changed=build.rs");

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
