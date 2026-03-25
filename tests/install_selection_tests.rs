use winbrew::core::install::selection::{
    SelectionContext, select_installer, select_installer_with_context,
};
use winbrew::manifest::InstallerEntry;

fn installer(architecture: &str, installer_type: &str, scope: Option<&str>) -> InstallerEntry {
    InstallerEntry {
        architecture: architecture.to_string(),
        installer_type: installer_type.to_string(),
        installer_url: format!("https://example.invalid/{architecture}/{installer_type}"),
        installer_sha256: "abc123".to_string(),
        installer_locale: None,
        scope: scope.map(ToOwned::to_owned),
        product_code: None,
        release_date: None,
        display_name: None,
        upgrade_behavior: None,
    }
}

#[test]
fn prefers_exact_architecture_over_fallbacks() {
    let context = SelectionContext::for_architecture("x64");
    let fallback_architecture = "x86";

    let installers = vec![
        installer(fallback_architecture, "exe", None),
        installer("x64", "msi", None),
    ];

    let selected = select_installer_with_context(&installers, &context)
        .expect("an installer should be selected");

    assert_eq!(selected.architecture, "x64");
    assert_eq!(selected.installer_type, "msi");
}

#[test]
fn prefers_user_scope_when_scores_are_close() {
    let context = SelectionContext::for_architecture("x64");
    let installers = vec![
        installer("x64", "msi", Some("Machine")),
        installer("x64", "msi", Some("User")),
    ];

    let selected = select_installer_with_context(&installers, &context)
        .expect("an installer should be selected");

    assert_eq!(selected.scope.as_deref(), Some("User"));
}

#[test]
fn keeps_manifest_order_when_scores_match() {
    let context = SelectionContext::for_architecture("x64");
    let installers = vec![installer("x64", "msi", None), installer("x64", "msi", None)];

    let selected = select_installer_with_context(&installers, &context)
        .expect("an installer should be selected");

    assert_eq!(selected.installer_type, "msi");
    assert_eq!(selected.installer_url, "https://example.invalid/x64/msi");
}

#[test]
fn select_installer_uses_current_system_context() {
    let installers = vec![installer("neutral", "portable", None)];

    let selected = select_installer(&installers).expect("installer should be selected");

    assert_eq!(selected.installer_type, "portable");
}
