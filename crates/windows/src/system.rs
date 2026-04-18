use std::fmt;
use std::mem::MaybeUninit;

use winbrew_models::domains::install::Architecture;
use windows_sys::Win32::System::SystemInformation::{
    GetNativeSystemInfo, PROCESSOR_ARCHITECTURE_AMD64, PROCESSOR_ARCHITECTURE_ARM64,
    PROCESSOR_ARCHITECTURE_INTEL, SYSTEM_INFO,
};
use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};

const PRODUCT_OPTIONS_KEY: &str = r"SYSTEM\CurrentControlSet\Control\ProductOptions";
const PRODUCT_TYPE_VALUE: &str = "ProductType";

/// Host family used for platform-aware installer selection.
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
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let product_type = hklm
        .open_subkey(PRODUCT_OPTIONS_KEY)
        .ok()
        .and_then(|key| key.get_value::<String, _>(PRODUCT_TYPE_VALUE).ok());

    product_type
        .as_deref()
        .map(classify_product_type)
        .unwrap_or(HostKind::Normal)
}

/// Return the native CPU architecture reported by Windows.
///
/// The helper uses `GetNativeSystemInfo` so the result reflects the host rather
/// than the current process emulation layer. Unknown processor architecture
/// codes fall back to `Architecture::Any`.
pub fn host_architecture() -> Architecture {
    let mut system_info = MaybeUninit::<SYSTEM_INFO>::uninit();

    unsafe {
        GetNativeSystemInfo(system_info.as_mut_ptr());
        architecture_from_native_system_info(system_info.assume_init())
    }
}

fn classify_product_type(product_type: &str) -> HostKind {
    match product_type.trim().to_ascii_lowercase().as_str() {
        "servernt" | "lanmannt" => HostKind::Server,
        _ => HostKind::Normal,
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
