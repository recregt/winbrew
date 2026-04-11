use strum_macros::Display;
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
};

const UNINSTALL: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
const WOW6432_UNINSTALL: &str =
    "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum Hive {
    #[strum(to_string = "HKLM")]
    LocalMachine,
    #[strum(to_string = "HKCU")]
    CurrentUser,
}

impl Hive {
    pub fn open(self) -> RegKey {
        let hkey = match self {
            Self::LocalMachine => HKEY_LOCAL_MACHINE,
            Self::CurrentUser => HKEY_CURRENT_USER,
        };
        RegKey::predef(hkey)
    }
}

pub struct UninstallRoot {
    pub hive: Hive,
    pub key_path: &'static str,
    pub key: RegKey,
    pub label: &'static str,
}

// return impl Iterator instead of allocating a Vec.
// caller decides whether to .collect() or just iterate lazily.
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
