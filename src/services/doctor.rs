use crate::models::Package;
use indicatif::ProgressBar;
use rayon::prelude::*;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnosis {
    pub package_name: String,
    pub install_dir: String,
    pub issue: String,
}

impl Diagnosis {
    pub fn check_package(pkg: &Package) -> Option<Self> {
        let install_dir = Path::new(&pkg.install_dir);

        if !install_dir.exists() {
            return Some(Self {
                package_name: pkg.name.clone(),
                install_dir: pkg.install_dir.clone(),
                issue: "missing install directory".to_string(),
            });
        }

        if !install_dir.is_dir() {
            return Some(Self {
                package_name: pkg.name.clone(),
                install_dir: pkg.install_dir.clone(),
                issue: "not a directory".to_string(),
            });
        }

        if std::fs::read_dir(install_dir).is_err() {
            return Some(Self {
                package_name: pkg.name.clone(),
                install_dir: pkg.install_dir.clone(),
                issue: "unreadable".to_string(),
            });
        }

        None
    }
}

pub fn scan_packages(packages: &[Package]) -> Vec<Diagnosis> {
    scan_packages_with_progress(packages, &ProgressBar::hidden())
}

pub fn scan_packages_with_progress(packages: &[Package], progress: &ProgressBar) -> Vec<Diagnosis> {
    progress.set_length(packages.len() as u64);
    progress.set_message("Scanning packages");

    let mut diagnoses: Vec<_> = packages
        .par_iter()
        .filter_map(|pkg| {
            let diagnosis = Diagnosis::check_package(pkg);
            progress.inc(1);
            diagnosis
        })
        .collect();

    diagnoses.sort_by(|left, right| left.package_name.cmp(&right.package_name));
    diagnoses
}
