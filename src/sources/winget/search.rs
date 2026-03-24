use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

use crate::core::network::http;
use crate::manifest::Manifest;
use crate::models::PackageCandidate;
use crate::sources::{winget_manifest_format, winget_registry_url, winget_repo_slug};

use super::manifest::manifest_url_for;
use super::parse_manifest;

const MAX_CANDIDATES: usize = 10;
const DEFAULT_BRANCH: &str = "master";

pub(crate) fn search_packages(query: &str) -> Result<Vec<PackageCandidate>> {
    if let Ok(Some(candidates)) = search_via_code_search(query)
        && !candidates.is_empty()
    {
        return Ok(candidates);
    }

    search_via_contents(query)
}

fn search_via_code_search(query: &str) -> Result<Option<Vec<PackageCandidate>>> {
    if crate::database::Config::current()
        .core
        .github_token
        .is_none()
    {
        return Ok(None);
    }

    let slug = winget_repo_slug().ok_or_else(|| {
        anyhow!("winget registry URL must point to a raw.githubusercontent.com repository root")
    })?;

    let search_url = reqwest::Url::parse_with_params(
        "https://api.github.com/search/code",
        [
            ("q", format!(r#"repo:{slug} \"{query}\" path:manifests/"#)),
            ("per_page", MAX_CANDIDATES.to_string()),
        ],
    )
    .context("failed to build GitHub search URL")?;

    let client = http::build_client()?;
    let search_url_string = search_url.to_string();
    let response =
        http::apply_github_auth(&search_url_string, client.get(search_url_string.clone()))?
            .send()
            .context("failed to search winget repository")?
            .error_for_status()
            .context("winget repository search failed")?
            .text()
            .context("failed to read winget search response")?;

    let results: SearchResponse =
        serde_json::from_str(&response).context("failed to parse winget search response")?;
    let candidates = candidates_from_search_items(&client, results.items)?;
    Ok(Some(candidates))
}

fn search_via_contents(query: &str) -> Result<Vec<PackageCandidate>> {
    let client = http::build_client()?;
    let listing_url = reqwest::Url::parse_with_params(
        "https://api.github.com/repos/microsoft/winget-pkgs/contents/manifests/m/Microsoft",
        [("ref", DEFAULT_BRANCH)],
    )
    .context("failed to build winget contents URL")?;

    let entries: Vec<ContentEntry> = fetch_contents(&client, listing_url.as_str())?;
    let query_terms = normalize_query(query);
    let mut candidates = Vec::new();

    for entry in entries.into_iter().filter(|entry| entry.kind == "dir") {
        if !matches_query(&entry.name, &query_terms) {
            continue;
        }

        if let Some(candidate) = candidate_from_package_dir(&client, &entry, &query_terms)? {
            candidates.push(candidate);
        }

        if candidates.len() >= MAX_CANDIDATES {
            break;
        }
    }

    Ok(candidates)
}

fn candidates_from_search_items(
    client: &reqwest::blocking::Client,
    items: Vec<SearchItem>,
) -> Result<Vec<PackageCandidate>> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let base = winget_registry_url();
    let format = winget_manifest_format();

    for item in items {
        let raw_url = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            item.path.trim_start_matches('/')
        );

        let content = http::apply_github_auth(&raw_url, client.get(&raw_url))?
            .send()
            .context("failed to fetch winget manifest")?
            .error_for_status()
            .context("winget manifest not found")?
            .text()
            .context("failed to read winget manifest")?;

        let manifest = parse_manifest(&format, &content)?;
        if !seen.insert(manifest.package.name.clone()) {
            continue;
        }

        candidates.push(package_candidate(&manifest, Some(item.path)));

        if candidates.len() >= MAX_CANDIDATES {
            break;
        }
    }

    Ok(candidates)
}

fn candidate_from_package_dir(
    client: &reqwest::blocking::Client,
    package_dir: &ContentEntry,
    query_terms: &str,
) -> Result<Option<PackageCandidate>> {
    let versions: Vec<ContentEntry> = fetch_contents(client, &package_dir.url)?;
    let Some(version) = latest_version_entry(&versions) else {
        return Ok(None);
    };

    let identifier = identifier_from_contents_path(&package_dir.path)?;
    let manifest_url = manifest_url_for(&identifier, &version.name)?;
    let format = winget_manifest_format();

    let content = http::apply_github_auth(&manifest_url, client.get(&manifest_url))?
        .send()
        .context("failed to fetch winget manifest")?
        .error_for_status()
        .context("winget manifest not found")?
        .text()
        .context("failed to read winget manifest")?;

    let manifest = parse_manifest(&format, &content)?;
    if !matches_query(&candidate_label_for(&manifest), query_terms) {
        return Ok(None);
    }

    Ok(Some(package_candidate(
        &manifest,
        Some(package_dir.path.clone()),
    )))
}

fn candidate_label_for(manifest: &Manifest) -> String {
    format!(
        "{} {} {}",
        manifest.package.name,
        manifest.package.package_name.as_deref().unwrap_or_default(),
        manifest.package.description.as_deref().unwrap_or_default()
    )
}

fn fetch_contents(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<ContentEntry>> {
    let content = http::apply_github_auth(url, client.get(url))?
        .send()
        .context("failed to fetch winget repository contents")?
        .error_for_status()
        .context("winget repository contents not found")?
        .text()
        .context("failed to read winget repository contents")?;

    if content.trim_start().starts_with('[') {
        let entries: Vec<ContentEntry> =
            serde_json::from_str(&content).context("failed to parse winget repository contents")?;
        return Ok(entries);
    }

    let entry: ContentEntry = serde_json::from_str(&content)
        .context("failed to parse winget repository contents entry")?;
    Ok(vec![entry])
}

fn latest_version_entry(entries: &[ContentEntry]) -> Option<&ContentEntry> {
    entries
        .iter()
        .filter(|entry| entry.kind == "dir" && entry.name.chars().any(|ch| ch.is_ascii_digit()))
        .max_by(|left, right| compare_versions(&left.name, &right.name))
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let left_parts = version_parts(left);
    let right_parts = version_parts(right);

    left_parts.cmp(&right_parts).then_with(|| left.cmp(right))
}

fn version_parts(version: &str) -> Vec<VersionPart> {
    version
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            segment
                .parse::<u64>()
                .map(VersionPart::Number)
                .unwrap_or_else(|_| VersionPart::Text(segment.to_ascii_lowercase()))
        })
        .collect()
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
enum VersionPart {
    Number(u64),
    Text(String),
}

fn identifier_from_contents_path(path: &str) -> Result<String> {
    let mut segments = path.split('/').filter(|segment| !segment.is_empty());
    let _root = segments.next();
    let _partition = segments.next();
    let publisher = segments
        .next()
        .ok_or_else(|| anyhow!("winget contents path is missing publisher segment"))?;
    let package = segments.collect::<Vec<_>>().join(".");

    if package.is_empty() {
        return Err(anyhow!("winget contents path is missing package segment"));
    }

    Ok(format!("{publisher}.{package}"))
}

fn matches_query(name: &str, query_terms: &str) -> bool {
    let normalized_name = normalize_name(name);
    let normalized_query = normalize_query(query_terms);

    normalized_query
        .split_whitespace()
        .all(|term| normalized_name.contains(term))
}

fn normalize_query(input: &str) -> String {
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

fn normalize_name(input: &str) -> String {
    let mut output = String::new();
    let mut previous_was_lowercase = false;

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && previous_was_lowercase && !output.ends_with(' ') {
                output.push(' ');
            }

            output.push(ch.to_ascii_lowercase());
            previous_was_lowercase = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            if !output.ends_with(' ') {
                output.push(' ');
            }

            previous_was_lowercase = false;
        }
    }

    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn package_candidate(manifest: &Manifest, manifest_path: Option<String>) -> PackageCandidate {
    PackageCandidate {
        identifier: manifest.package.name.clone(),
        package_name: manifest.package.package_name.clone(),
        version: manifest.package.version.clone(),
        description: manifest.package.description.clone(),
        publisher: manifest.package.publisher.clone(),
        manifest_path,
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    items: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    path: String,
}

#[derive(Debug, Deserialize)]
struct ContentEntry {
    name: String,
    path: String,
    #[serde(rename = "type")]
    kind: String,
    url: String,
}
