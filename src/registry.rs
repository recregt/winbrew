use anyhow::Result;

use crate::uninstall::uninstall_roots;

pub fn show_installed_apps(filter: Option<&str>) -> Result<()> {
    let filter_lower = filter.map(|f| f.to_lowercase());

    for root in uninstall_roots() {
        print_apps(&root.key, filter_lower.as_deref());
    }

    Ok(())
}

fn print_apps(key: &winreg::RegKey, filter_lower: Option<&str>) {
    key.enum_keys()
        .flatten()
        .filter_map(|name| key.open_subkey(&name).ok())
        .filter_map(|app_key| app_key.get_value::<String, _>("DisplayName").ok())
        .filter(|name| matches_filter(name, filter_lower))
        .for_each(|name| println!("{name}"));
}

fn matches_filter(name: &str, filter_lower: Option<&str>) -> bool {
    match filter_lower {
        Some(filter) => name.to_lowercase().contains(filter),
        None => true,
    }
}
