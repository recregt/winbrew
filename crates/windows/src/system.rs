use std::fmt;

use crate::models::domains::install::Architecture;
use crate::registry::read_product_type;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::SystemInformation::{
    GetNativeSystemInfo, PROCESSOR_ARCHITECTURE_AMD64, PROCESSOR_ARCHITECTURE_ARM64,
    PROCESSOR_ARCHITECTURE_INTEL, SYSTEM_INFO,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const NORMAL_PLATFORM_TAGS: &[&str] = &["windows.desktop", "windows.ltsc", "windows.universal"];
const SERVER_PLATFORM_TAGS: &[&str] = &["windows.server"];

/// Combined host family and native architecture snapshot used for installer selection.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostProfile {
    /// `true` when Windows reports a server-class product type.
    pub is_server: bool,
    /// Native CPU architecture reported by Windows.
    pub architecture: Architecture,
}

impl HostProfile {
    /// Return the Winget platform labels accepted for this host profile.
    pub fn platform_tags(self) -> &'static [&'static str] {
        if self.is_server {
            SERVER_PLATFORM_TAGS
        } else {
            NORMAL_PLATFORM_TAGS
        }
    }
}

impl fmt::Display for HostProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let host_family = if self.is_server { "server" } else { "normal" };

        write!(f, "{host_family} {}", self.architecture)
    }
}

/// Return the current Windows host profile for platform-aware catalog selection.
///
/// The helper is best-effort: if the Windows product-type registry key cannot
/// be read, the host family falls back to `normal` instead of blocking install
/// flows.
pub fn host_profile() -> HostProfile {
    HostProfile {
        is_server: read_product_type()
            .as_deref()
            .map(classify_product_type)
            .unwrap_or(false),
        architecture: native_architecture(),
    }
}

/// Return `true` when the current process is running elevated.
pub fn is_elevated() -> bool {
    let mut token = core::ptr::null_mut();

    let token_opened =
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } != 0;
    if !token_opened {
        return false;
    }

    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut return_length = 0u32;
    let queried = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut TOKEN_ELEVATION as *mut core::ffi::c_void,
            core::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        )
    } != 0;

    unsafe {
        CloseHandle(token);
    }

    queried && elevation.TokenIsElevated != 0
}

fn classify_product_type(product_type: &str) -> bool {
    let trimmed = product_type.trim();

    trimmed.eq_ignore_ascii_case("servernt") || trimmed.eq_ignore_ascii_case("lanmannt")
}

fn native_architecture() -> Architecture {
    let mut system_info: SYSTEM_INFO = unsafe { std::mem::zeroed() };

    unsafe {
        GetNativeSystemInfo(&mut system_info as *mut SYSTEM_INFO);
    }

    architecture_from_native_system_info(system_info)
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
    use super::{HostProfile, architecture_from_native_system_info, classify_product_type};
    use crate::models::domains::install::Architecture;
    use windows_sys::Win32::System::SystemInformation::{
        PROCESSOR_ARCHITECTURE_AMD64, PROCESSOR_ARCHITECTURE_ARM64, PROCESSOR_ARCHITECTURE_INTEL,
        SYSTEM_INFO,
    };

    fn system_info_for(architecture: u16) -> SYSTEM_INFO {
        let mut system_info = unsafe { std::mem::zeroed::<SYSTEM_INFO>() };
        system_info.Anonymous.Anonymous.wProcessorArchitecture = architecture;
        system_info
    }

    fn host_profile(is_server: bool, architecture: Architecture) -> HostProfile {
        HostProfile {
            is_server,
            architecture,
        }
    }

    #[test]
    fn classifies_server_product_types() {
        assert!(classify_product_type("ServerNT"));
        assert!(classify_product_type("LanmanNT"));
        assert!(!classify_product_type("WinNT"));
    }

    #[test]
    fn classifies_server_product_types_case_insensitively_and_with_whitespace() {
        assert!(classify_product_type("  SERVERNT  "));
        assert!(classify_product_type("  lanmannt"));
        assert!(!classify_product_type(""));
        assert!(!classify_product_type("Unknown"));
    }

    #[test]
    fn host_profile_exposes_platform_tags_by_family() {
        assert_eq!(
            host_profile(false, Architecture::X64).platform_tags(),
            &["windows.desktop", "windows.ltsc", "windows.universal"]
        );
        assert_eq!(
            host_profile(true, Architecture::Arm64).platform_tags(),
            &["windows.server"]
        );
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
