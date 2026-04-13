use std::collections::HashMap;
use std::path::Path;

use crate::core::paths::ResolvedPaths;
use crate::models::domains::installed::InstalledPackage;
use crate::models::domains::reporting::{DiagnosisResult, DiagnosisSeverity};
use crate::storage::database;

use super::{PackageJournalScan, sort_diagnoses, sort_recovery_findings};

/// Scan package journal files under `data/pkgdb` and report recovery issues.
pub(super) fn scan_package_journals(
    paths: &ResolvedPaths,
    packages: &[InstalledPackage],
) -> PackageJournalScan {
    let pkgdb_root = paths.pkgdb.clone();

    if !pkgdb_root.exists() {
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
                return PackageJournalScan::new();
            }

            let mut result = PackageJournalScan::new();
            result.push(
                DiagnosisResult {
                    error_code: "pkgdb_unreadable".to_string(),
                    description: format!(
                        "pkgdb root: unreadable journal directory ({}) - {err}",
                        pkgdb_root.to_string_lossy()
                    ),
                    severity: DiagnosisSeverity::Error,
                },
                None,
            );
            return result;
        }
    };

    let mut result = PackageJournalScan::new();

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };

        if !file_type.is_dir() {
            continue;
        }

        let journal_path = entry.path().join("journal.jsonl");
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
            Err(database::JournalReadError::Incomplete { .. }) => {
                result.push(
                    DiagnosisResult {
                        error_code: "incomplete_package_journal".to_string(),
                        description: format!(
                            "{}: incomplete recovery journal",
                            journal_path.to_string_lossy()
                        ),
                        severity: DiagnosisSeverity::Error,
                    },
                    None,
                );
            }
            Err(database::JournalReadError::Read { .. }) => {
                result.push(
                    DiagnosisResult {
                        error_code: "unreadable_package_journal".to_string(),
                        description: format!(
                            "{}: recovery journal is unreadable",
                            journal_path.to_string_lossy()
                        ),
                        severity: DiagnosisSeverity::Error,
                    },
                    None,
                );
            }
            Err(database::JournalReadError::MalformedLine { line, .. }) => {
                result.push(
                    DiagnosisResult {
                        error_code: "malformed_package_journal".to_string(),
                        description: format!(
                            "{}: recovery journal has malformed line {line}",
                            journal_path.to_string_lossy()
                        ),
                        severity: DiagnosisSeverity::Error,
                    },
                    None,
                );
            }
            Err(database::JournalReadError::TrailingEntries { line, .. }) => {
                result.push(
                    DiagnosisResult {
                        error_code: "trailing_package_journal".to_string(),
                        description: format!(
                            "{}: recovery journal has trailing entries after commit on line {line}",
                            journal_path.to_string_lossy()
                        ),
                        severity: DiagnosisSeverity::Error,
                    },
                    Some(&journal_path),
                );
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
    let Some((package_id, version, engine, install_dir)) =
        entries.iter().find_map(|entry| match entry {
            database::JournalEntry::Metadata {
                package_id,
                version,
                engine,
                install_dir,
                dependencies: _,
                engine_metadata: _,
            } => Some((
                package_id.as_str(),
                version.as_str(),
                engine.as_str(),
                install_dir.as_str(),
            )),
            _ => None,
        })
    else {
        return vec![DiagnosisResult {
            error_code: "missing_journal_metadata".to_string(),
            description: format!(
                "{}: committed recovery journal is missing metadata",
                journal_path.to_string_lossy()
            ),
            severity: DiagnosisSeverity::Error,
        }];
    };

    let Some(package) = packages.get(package_id) else {
        return vec![DiagnosisResult {
            error_code: "orphan_package_journal".to_string(),
            description: format!(
                "{}: committed recovery journal has no installed package",
                journal_path.to_string_lossy()
            ),
            severity: DiagnosisSeverity::Warning,
        }];
    };

    if package.version != version
        || !package.engine_kind.as_str().eq_ignore_ascii_case(engine)
        || package.install_dir != install_dir
    {
        return vec![DiagnosisResult {
            error_code: "stale_package_journal".to_string(),
            description: format!(
                "{}: recovery journal does not match installed package {} ({})",
                journal_path.to_string_lossy(),
                package.name,
                package.version
            ),
            severity: DiagnosisSeverity::Warning,
        }];
    }

    Vec::new()
}
