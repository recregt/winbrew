use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
};

pub(super) const UNINSTALL: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
pub(super) const WOW6432_UNINSTALL: &str =
    "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

/// Registry hive that can contain uninstall data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hive {
    /// `HKEY_LOCAL_MACHINE`.
    LocalMachine,
    /// `HKEY_CURRENT_USER`.
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
#[derive(Debug)]
pub(super) struct UninstallRoot {
    key: RegKey,
}

impl UninstallRoot {
    /// Open registry key handle for the uninstall root.
    pub(super) fn key(&self) -> &RegKey {
        &self.key
    }

    fn new(key: RegKey) -> Self {
        Self { key }
    }
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
pub(super) fn uninstall_roots() -> impl Iterator<Item = UninstallRoot> {
    [
        (Hive::LocalMachine, UNINSTALL),
        (Hive::LocalMachine, WOW6432_UNINSTALL),
        (Hive::CurrentUser, UNINSTALL),
    ]
    .into_iter()
    .filter_map(|(hive, key_path)| {
        hive.open()
            .open_subkey(key_path)
            .ok()
            .map(UninstallRoot::new)
    })
}
