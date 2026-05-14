#![cfg(windows)]

#[test]
fn msix_adapter_exports_are_visible() {
    let install_fn = winbrew_engines::windows::msix::install;
    let remove_fn = winbrew_engines::windows::msix::remove;
    let _ = (install_fn, remove_fn);
}
