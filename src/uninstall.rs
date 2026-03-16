use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::{HKCU, HKLM, RegKey};

const UNINSTALL: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
const WOW_UNINSTALL: &str = "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

#[derive(Clone, Copy)]
pub enum Hive {
    LocalMachine,
    CurrentUser,
}

impl Hive {
    pub fn open(self) -> RegKey {
        match self {
            Self::LocalMachine => RegKey::predef(HKEY_LOCAL_MACHINE),
            Self::CurrentUser => RegKey::predef(HKEY_CURRENT_USER),
        }
    }
}

pub struct UninstallRoot {
    pub hive: Hive,
    pub key_path: &'static str,
    pub key: RegKey,
    pub label: &'static str,
}

pub fn uninstall_roots() -> Vec<UninstallRoot> {
    [
        HKLM.open_subkey(UNINSTALL).ok().map(|key| UninstallRoot {
            hive: Hive::LocalMachine,
            key_path: UNINSTALL,
            key,
            label: "HKLM\\Uninstall",
        }),
        HKLM.open_subkey(WOW_UNINSTALL)
            .ok()
            .map(|key| UninstallRoot {
                hive: Hive::LocalMachine,
                key_path: WOW_UNINSTALL,
                key,
                label: "HKLM\\WOW6432Node\\Uninstall",
            }),
        HKCU.open_subkey(UNINSTALL).ok().map(|key| UninstallRoot {
            hive: Hive::CurrentUser,
            key_path: UNINSTALL,
            key,
            label: "HKCU\\Uninstall",
        }),
    ]
    .into_iter()
    .flatten()
    .collect()
}
