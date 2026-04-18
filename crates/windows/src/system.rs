use std::fmt;

use crate::registry::read_product_type;
use winbrew_models::domains::install::Architecture;
use windows_sys::Win32::System::SystemInformation::{
    GetNativeSystemInfo, PROCESSOR_ARCHITECTURE_AMD64, PROCESSOR_ARCHITECTURE_ARM64,
    PROCESSOR_ARCHITECTURE_INTEL, SYSTEM_INFO,
};

/// Host family used for platform-aware installer selection.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    /// Client, workstation, or desktop Windows.
    Normal,
    /// Server-class Windows.
    Server,
}

impl HostKind {
    /// Return the Winget platform labels accepted for this host family.
    pub fn platform_tags(self) -> &'static [&'static str] {
        match self {
            Self::Normal => &["windows.desktop", "windows.ltsc"],
            Self::Server => &["windows.server"],
        }
    }
}

impl fmt::Display for HostKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Normal => "normal",
            Self::Server => "server",
        })
    }
}

/// Return the current host family for platform-aware catalog selection.
///
/// The helper is best-effort: if the Windows product-type registry key cannot
/// be read, the function falls back to `HostKind::Normal` instead of blocking
/// install flows.
pub fn host_kind() -> HostKind {
    read_product_type()
        .as_deref()
        .map(classify_product_type)
        .unwrap_or(HostKind::Normal)
}

/// Return the native CPU architecture reported by Windows.
///
/// The helper uses `GetNativeSystemInfo` so the result reflects the host rather
/// than the current process emulation layer. Unknown processor architecture
/// codes fall back to `Architecture::Any`.
#[must_use]
pub fn host_architecture() -> Architecture {
    let mut system_info: SYSTEM_INFO = unsafe { std::mem::zeroed() };

    unsafe {
        GetNativeSystemInfo(&mut system_info as *mut SYSTEM_INFO);
    }

    architecture_from_native_system_info(system_info)
}

fn classify_product_type(product_type: &str) -> HostKind {
    let trimmed = product_type.trim();

    if trimmed.eq_ignore_ascii_case("servernt") || trimmed.eq_ignore_ascii_case("lanmannt") {
        HostKind::Server
    } else {
        HostKind::Normal
    }
}

fn architecture_from_native_system_info(system_info: SYSTEM_INFO) -> Architecture {
    let processor_architecture = unsafe { system_info.Anonymous.Anonymous.wProcessorArchitecture };

    match processor_architecture {
        PROCESSOR_ARCHITECTURE_AMD64 => Architecture::X64,
        PROCESSOR_ARCHITECTURE_INTEL => Architecture::X86,
        PROCESSOR_ARCHITECTURE_ARM64 => Architecture::Arm64,
        _ => Architecture::Any,
    }
}

#[cfg(test)]
mod tests {
    use super::{HostKind, architecture_from_native_system_info, classify_product_type};
    use winbrew_models::domains::install::Architecture;
    use windows_sys::Win32::System::SystemInformation::{
        PROCESSOR_ARCHITECTURE_AMD64, PROCESSOR_ARCHITECTURE_ARM64, PROCESSOR_ARCHITECTURE_INTEL,
        SYSTEM_INFO,
    };

    fn system_info_for(architecture: u16) -> SYSTEM_INFO {
        let mut system_info = unsafe { std::mem::zeroed::<SYSTEM_INFO>() };
        system_info.Anonymous.Anonymous.wProcessorArchitecture = architecture;
        system_info
    }

    #[test]
    fn classifies_server_product_types() {
        assert_eq!(classify_product_type("ServerNT"), HostKind::Server);
        assert_eq!(classify_product_type("LanmanNT"), HostKind::Server);
        assert_eq!(classify_product_type("WinNT"), HostKind::Normal);
    }

    #[test]
    fn classifies_server_product_types_case_insensitively_and_with_whitespace() {
        assert_eq!(classify_product_type("  SERVERNT  "), HostKind::Server);
        assert_eq!(classify_product_type("  lanmannt"), HostKind::Server);
        assert_eq!(classify_product_type(""), HostKind::Normal);
        assert_eq!(classify_product_type("Unknown"), HostKind::Normal);
    }

    #[test]
    fn maps_native_processor_architectures() {
        assert_eq!(
            architecture_from_native_system_info(system_info_for(PROCESSOR_ARCHITECTURE_AMD64)),
            Architecture::X64
        );
        assert_eq!(
            architecture_from_native_system_info(system_info_for(PROCESSOR_ARCHITECTURE_INTEL)),
            Architecture::X86
        );
        assert_eq!(
            architecture_from_native_system_info(system_info_for(PROCESSOR_ARCHITECTURE_ARM64)),
            Architecture::Arm64
        );
        assert_eq!(
            architecture_from_native_system_info(system_info_for(0xffff)),
            Architecture::Any
        );
    }
}
