use crate::AppContext;
pub use winbrew_install::{
    CatalogPackage, InstallError, InstallFailureClass, InstallObserver, InstallOutcome,
    InstallResult, PackageRef, Result,
};
pub use winbrew_install::{download, flow, state, types};

pub fn run<O: InstallObserver>(
    ctx: &AppContext,
    package_ref: PackageRef,
    ignore_checksum_security: bool,
    observer: &mut O,
) -> Result<InstallOutcome> {
    winbrew_install::run(&ctx.paths, package_ref, ignore_checksum_security, observer)
}
