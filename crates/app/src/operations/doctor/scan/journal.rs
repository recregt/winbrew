use std::collections::HashMap;
use std::path::Path;

use crate::core::paths::ResolvedPaths;
use crate::database;
use crate::models::domains::installed::InstalledPackage;
use crate::models::domains::reporting::{DiagnosisResult, DiagnosisSeverity};
use tracing::debug;

use super::{PackageJournalScan, sort_diagnoses, sort_recovery_findings};

fn diagnosis(
    error_code: &str,
    description: String,
    severity: DiagnosisSeverity,
) -> DiagnosisResult {
    DiagnosisResult {
        error_code: error_code.to_string(),
        description,
        severity,
    }
}

fn journal_error(
    journal_path: &Path,
    error_code: &str,
    message: impl std::fmt::Display,
    severity: DiagnosisSeverity,
) -> DiagnosisResult {
    diagnosis(
        error_code,
        format!("{}: {}", journal_path.to_string_lossy(), message),
        severity,
    )
}

fn journal_read_error_diagnosis(
    journal_path: &Path,
    error: database::JournalReadError,
) -> (DiagnosisResult, Option<&Path>) {
    match error {
        database::JournalReadError::Incomplete { .. } => (
            journal_error(
                journal_path,
                "incomplete_package_journal",
                "incomplete recovery journal",
                DiagnosisSeverity::Error,
            ),
            None,
        ),
        database::JournalReadError::Read { .. } => (
            journal_error(
                journal_path,
                "unreadable_package_journal",
                "recovery journal is unreadable",
                DiagnosisSeverity::Error,
            ),
            None,
        ),
        database::JournalReadError::MalformedLine { line, .. } => (
            journal_error(
                journal_path,
                "malformed_package_journal",
                format!("recovery journal has malformed line {line}"),
                DiagnosisSeverity::Error,
            ),
            None,
        ),
        database::JournalReadError::TrailingEntries { line, .. } => (
            journal_error(
                journal_path,
                "trailing_package_journal",
                format!("recovery journal has trailing entries after commit on line {line}"),
                DiagnosisSeverity::Error,
            ),
            Some(journal_path),
        ),
    }
}

/// Scan package journal files under `data/pkgdb` and report recovery issues.
pub(super) fn scan_package_journals(
    paths: &ResolvedPaths,
    packages: &[InstalledPackage],
) -> PackageJournalScan {
    let pkgdb_root = paths.pkgdb.clone();

    if !pkgdb_root.exists() {
        debug!(path = %pkgdb_root.display(), "pkgdb root does not exist, skipping journal scan");
        return PackageJournalScan::new();
    }

    let package_lookup: HashMap<&str, &InstalledPackage> = packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect();

    let entries = match std::fs::read_dir(&pkgdb_root) {
        Ok(entries) => entries,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                debug!(path = %pkgdb_root.display(), "pkgdb root disappeared before journal scan");
                return PackageJournalScan::new();
            }

            let mut result = PackageJournalScan::new();
            result.push(
                diagnosis(
                    "pkgdb_unreadable",
                    format!(
                        "pkgdb root: unreadable journal directory ({}) - {err}",
                        pkgdb_root.to_string_lossy()
                    ),
                    DiagnosisSeverity::Error,
                ),
                None,
            );
            return result;
        }
    };

    let mut result = PackageJournalScan::new();

    for entry_result in entries {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(err) => {
                debug!(path = %pkgdb_root.display(), error = %err, "skipping unreadable pkgdb entry");
                continue;
            }
        };

        let entry_path = entry.path();

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) => {
                debug!(path = %entry_path.display(), error = %err, "skipping pkgdb entry with unreadable file type");
                continue;
            }
        };

        if !file_type.is_dir() {
            continue;
        }

        let journal_path = entry_path.join("journal.jsonl");
        if !journal_path.exists() {
            continue;
        }

        match database::JournalReader::read_committed(&journal_path) {
            Ok(entries) => {
                for diagnosis in
                    diagnose_committed_journal(&journal_path, &entries, &package_lookup)
                {
                    result.push(diagnosis, Some(&journal_path));
                }
            }
            Err(error) => {
                let (diagnosis, target_path) = journal_read_error_diagnosis(&journal_path, error);
                result.push(diagnosis, target_path);
            }
        }
    }

    result.diagnostics = sort_diagnoses(result.diagnostics);
    result
        .recovery_findings
        .sort_unstable_by(sort_recovery_findings);

    result
}

fn diagnose_committed_journal(
    journal_path: &Path,
    entries: &[database::JournalEntry],
    packages: &HashMap<&str, &InstalledPackage>,
) -> Vec<DiagnosisResult> {
    let Some((package_id, version, engine, deployment_kind, install_dir)) =
        entries.iter().find_map(|entry| match entry {
            database::JournalEntry::Metadata {
                package_id,
                version,
                engine,
                deployment_kind,
                install_dir,
                dependencies: _,
                engine_metadata: _,
            } => Some((
                package_id.as_str(),
                version.as_str(),
                engine.as_str(),
                *deployment_kind,
                install_dir.as_str(),
            )),
            _ => None,
        })
    else {
        return vec![journal_error(
            journal_path,
            "missing_journal_metadata",
            "committed recovery journal is missing metadata",
            DiagnosisSeverity::Error,
        )];
    };

    let Some(package) = packages.get(package_id) else {
        return vec![journal_error(
            journal_path,
            "orphan_package_journal",
            "committed recovery journal has no installed package",
            DiagnosisSeverity::Warning,
        )];
    };

    if package.version != version
        || !package.engine_kind.as_str().eq_ignore_ascii_case(engine)
        || package.install_dir != install_dir
        || package.deployment_kind != deployment_kind
    {
        return vec![journal_error(
            journal_path,
            "stale_package_journal",
            format!(
                "recovery journal does not match installed package {} ({})",
                package.name, package.version
            ),
            DiagnosisSeverity::Warning,
        )];
    }

    Vec::new()
}
