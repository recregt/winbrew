use anyhow::{Context, Result, bail};
use windows::ApplicationModel::Package;
use windows::Management::Deployment::PackageManager;
use windows::core::HSTRING;

pub fn remove(package_name: &str) -> Result<()> {
    let package_manager = PackageManager::new().context("failed to create package manager")?;
    let matching_full_names = matching_package_full_names(&package_manager, package_name)?;

    if matching_full_names.is_empty() {
        bail!("no installed msix package matched '{package_name}'");
    }

    for full_name in matching_full_names {
        package_manager
            .RemovePackageAsync(&full_name)
            .with_context(|| format!("failed to start uninstall for {full_name}"))?
            .get()
            .with_context(|| format!("msix uninstall failed for {full_name}"))?;
    }

    Ok(())
}

fn matching_package_full_names(
    package_manager: &PackageManager,
    package_name: &str,
) -> Result<Vec<HSTRING>> {
    let normalized_name = package_name.trim().to_ascii_lowercase();
    let mut matching_full_names = Vec::new();

    if let Ok(package) = package_manager.FindPackageByPackageFullName(&HSTRING::from(package_name))
    {
        if package_matches(&package, &normalized_name)? {
            matching_full_names.push(package_full_name(&package)?);
        }
    }

    for package in package_manager
        .FindPackagesByPackageFamilyName(&HSTRING::from(package_name))
        .context("failed to enumerate installed packages")?
    {
        if package_matches(&package, &normalized_name)? {
            matching_full_names.push(package_full_name(&package)?);
        }
    }

    matching_full_names.sort_by_key(|value| value.to_string());
    matching_full_names.dedup_by(|left, right| left.to_string() == right.to_string());

    Ok(matching_full_names)
}

fn package_matches(package: &Package, expected_name: &str) -> Result<bool> {
    let package_id = package.Id().context("failed to read package identity")?;

    Ok(identity_matches(
        &package_id
            .Name()
            .context("failed to read package name")?
            .to_string(),
        &package_id
            .FamilyName()
            .context("failed to read package family name")?
            .to_string(),
        &package_id
            .FullName()
            .context("failed to read package full name")?
            .to_string(),
        expected_name,
    ))
}

fn package_full_name(package: &Package) -> Result<HSTRING> {
    package
        .Id()
        .context("failed to read package identity")?
        .FullName()
        .context("failed to read package full name")
}

fn identity_matches(name: &str, family_name: &str, full_name: &str, expected_name: &str) -> bool {
    [name, family_name, full_name]
        .into_iter()
        .any(|value| value.eq_ignore_ascii_case(expected_name))
}

#[cfg(test)]
mod tests {
    use super::identity_matches;

    #[test]
    fn identity_matches_name_family_or_full_name() {
        assert!(identity_matches(
            "Contoso.App",
            "Contoso.App_123abc",
            "Contoso.App_123abc!App",
            "contoso.app"
        ));
        assert!(identity_matches(
            "Contoso.App",
            "Contoso.App_123abc",
            "Contoso.App_123abc!App",
            "contoso.app_123abc"
        ));
        assert!(identity_matches(
            "Contoso.App",
            "Contoso.App_123abc",
            "Contoso.App_123abc!App",
            "contoso.app_123abc!app"
        ));
    }

    #[test]
    fn identity_matches_rejects_other_names() {
        assert!(!identity_matches(
            "Contoso.App",
            "Contoso.App_123abc",
            "Contoso.App_123abc!App",
            "fabrikam.tool"
        ));
    }
}
