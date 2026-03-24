use anyhow::{Result, bail};

use crate::models::{PackageCandidate, PackageQuery};
use crate::sources;

#[derive(Debug, Clone)]
pub struct ResolvedInstall {
    pub identifier: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub enum Resolution {
    Resolved(ResolvedInstall),
    Candidates(Vec<PackageCandidate>),
}

pub fn resolve(query: &[String], version: Option<&str>) -> Result<Resolution> {
    let query = PackageQuery {
        terms: query.to_vec(),
        version: version.map(ToOwned::to_owned),
    };

    let query_text = query.text();
    if is_canonical_identifier(&query_text)
        && let Some(version) = query.version.clone()
    {
        return Ok(Resolution::Resolved(ResolvedInstall {
            identifier: query_text,
            version,
        }));
    }

    // Fall through to search so we can discover the latest manifest version.

    let source = sources::active_source()?;
    let candidates = source.search_packages(&query_text)?;

    if candidates.is_empty() {
        bail!("no packages matched: {query_text}");
    }

    let mut ranked = candidates.to_vec();
    ranked.sort_by(|left, right| {
        candidate_score(&query_text, left)
            .cmp(&candidate_score(&query_text, right))
            .then_with(|| candidate_label(left).cmp(&candidate_label(right)))
    });

    if let Some(candidate) = ranked.first()
        && candidate_score(&query_text, candidate) == 0
    {
        return Ok(Resolution::Resolved(ResolvedInstall {
            identifier: candidate.identifier.clone(),
            version: query.version.unwrap_or_else(|| candidate.version.clone()),
        }));
    }

    if ranked.len() == 1 {
        let candidate = ranked.remove(0);
        return Ok(Resolution::Resolved(ResolvedInstall {
            identifier: candidate.identifier,
            version: query.version.unwrap_or(candidate.version),
        }));
    }

    Ok(Resolution::Candidates(ranked))
}

fn candidate_score(query: &str, candidate: &PackageCandidate) -> usize {
    let query = normalize(query);
    let identifier = normalize(&candidate.identifier);
    let package_name = candidate.package_name.as_deref().map(normalize);
    let description = candidate.description.as_deref().map(normalize);
    let publisher = candidate.publisher.as_deref().map(normalize);

    if identifier == query {
        return 0;
    }

    if package_name.as_deref() == Some(query.as_str()) {
        return 0;
    }

    if description.as_deref() == Some(query.as_str()) {
        return 0;
    }

    if publisher.as_deref() == Some(query.as_str()) {
        return 0;
    }

    if identifier.starts_with(&query)
        || package_name
            .as_deref()
            .is_some_and(|name| name.starts_with(&query))
    {
        return 1;
    }

    let query_terms: Vec<&str> = query.split_whitespace().collect();
    let haystack = [
        Some(identifier.as_str()),
        package_name.as_deref(),
        description.as_deref(),
        publisher.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");

    if query_terms.iter().all(|term| haystack.contains(term)) {
        return 10;
    }

    if query_terms.iter().any(|term| haystack.contains(term)) {
        return 20;
    }

    100
}

fn is_canonical_identifier(query: &str) -> bool {
    let parts: Vec<&str> = query.split('.').collect();
    parts.len() >= 2 && parts.iter().all(|part| !part.trim().is_empty()) && !query.contains(' ')
}

fn candidate_label(candidate: &PackageCandidate) -> String {
    format!(
        "{}:{}",
        candidate
            .package_name
            .as_deref()
            .unwrap_or(&candidate.identifier),
        candidate.identifier
    )
}

fn normalize(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch.is_whitespace() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::fs;
    use std::path::PathBuf;

    fn candidate(
        identifier: &str,
        package_name: Option<&str>,
        description: Option<&str>,
        publisher: Option<&str>,
    ) -> PackageCandidate {
        PackageCandidate {
            identifier: identifier.to_string(),
            package_name: package_name.map(ToOwned::to_owned),
            version: "1.0.0".to_string(),
            description: description.map(ToOwned::to_owned),
            publisher: publisher.map(ToOwned::to_owned),
            manifest_path: None,
        }
    }

    #[derive(Debug, Deserialize)]
    struct CandidateFixture {
        identifier: String,
        package_name: Option<String>,
        version: String,
        description: Option<String>,
        publisher: Option<String>,
        manifest_path: Option<String>,
    }

    fn fixture_path(file_name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("winget")
            .join(file_name)
    }

    #[test]
    fn exact_match_is_ranked_before_partial_matches() {
        let query = "windows terminal";
        let exact = candidate(
            "Microsoft.WindowsTerminal",
            Some("Windows Terminal"),
            Some("Windows Terminal is a modern terminal application."),
            Some("Microsoft Corporation"),
        );
        let partial = candidate(
            "Microsoft.WindowsTerminalPreview",
            Some("Windows Terminal Preview"),
            Some("Preview channel for Windows Terminal."),
            Some("Microsoft Corporation"),
        );

        assert_eq!(candidate_score(query, &exact), 0);
        assert!(candidate_score(query, &partial) > 0);

        let mut ranked = [partial.clone(), exact.clone()];
        ranked.sort_by(|left, right| {
            candidate_score(query, left)
                .cmp(&candidate_score(query, right))
                .then_with(|| candidate_label(left).cmp(&candidate_label(right)))
        });

        assert_eq!(ranked[0].identifier, exact.identifier);
    }

    #[test]
    fn normalizes_punctuation_and_case_in_queries() {
        let query = "Windows-Terminal";
        let candidate = candidate(
            "Microsoft.WindowsTerminal",
            Some("Windows Terminal"),
            Some("Terminal application"),
            Some("Microsoft Corporation"),
        );

        assert_eq!(candidate_score(query, &candidate), 0);
    }

    #[test]
    fn ranks_candidates_loaded_from_disk() {
        let content = fs::read_to_string(fixture_path("windows-terminal.candidates.json"))
            .expect("fixture file should exist");
        let fixture: Vec<CandidateFixture> =
            serde_json::from_str(&content).expect("fixture should parse");
        let mut candidates: Vec<PackageCandidate> = fixture
            .into_iter()
            .map(|entry| PackageCandidate {
                identifier: entry.identifier,
                package_name: entry.package_name,
                version: entry.version,
                description: entry.description,
                publisher: entry.publisher,
                manifest_path: entry.manifest_path,
            })
            .collect();

        let query = "windows terminal";
        candidates.sort_by(|left, right| {
            candidate_score(query, left)
                .cmp(&candidate_score(query, right))
                .then_with(|| candidate_label(left).cmp(&candidate_label(right)))
        });

        assert_eq!(candidates[0].identifier, "Microsoft.WindowsTerminal");
        assert_eq!(candidate_score(query, &candidates[0]), 0);
    }
}
