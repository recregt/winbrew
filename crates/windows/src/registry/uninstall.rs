use strum_macros::Display;
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
};

const UNINSTALL: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
const WOW6432_UNINSTALL: &str =
    "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

/// Registry hive that can contain uninstall data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum Hive {
    /// `HKEY_LOCAL_MACHINE`.
    #[strum(to_string = "HKLM")]
    LocalMachine,
    /// `HKEY_CURRENT_USER`.
    #[strum(to_string = "HKCU")]
    CurrentUser,
}

impl Hive {
    /// Open the registry root associated with this hive.
    pub fn open(self) -> RegKey {
        let hkey = match self {
            Self::LocalMachine => HKEY_LOCAL_MACHINE,
            Self::CurrentUser => HKEY_CURRENT_USER,
        };
        RegKey::predef(hkey)
    }
}

/// Snapshot of one uninstall registry location.
pub struct UninstallRoot {
    /// Hive that owns the root key.
    pub hive: Hive,
    /// Relative registry path under the hive.
    pub key_path: &'static str,
    /// Open registry key handle for the uninstall root.
    pub key: RegKey,
    /// Display label used in diagnostics and logs.
    pub label: &'static str,
}

/// Iterate over the uninstall roots that exist on the current machine.
///
/// The iterator includes the standard machine, WOW6432Node, and user uninstall
/// locations when they are present. Missing roots are skipped, so callers can
/// iterate lazily without allocating a collection first.
///
/// # Example
///
/// ```no_run
/// use winbrew_windows::uninstall_roots;
///
/// for root in uninstall_roots() {
///     println!("{} -> {}", root.hive, root.label);
/// }
/// ```
pub fn uninstall_roots() -> impl Iterator<Item = UninstallRoot> {
    [
        (Hive::LocalMachine, UNINSTALL, "HKLM\\Uninstall"),
        (
            Hive::LocalMachine,
            WOW6432_UNINSTALL,
            "HKLM\\WOW6432Node\\Uninstall",
        ),
        (Hive::CurrentUser, UNINSTALL, "HKCU\\Uninstall"),
    ]
    .into_iter()
    .filter_map(|(hive, key_path, label)| {
        hive.open()
            .open_subkey(key_path)
            .ok()
            .map(|key| UninstallRoot {
                hive,
                key_path,
                key,
                label,
            })
    })
}
