use core::fmt;
use core::str::FromStr;

use semver as semver_crate;
use serde::{Deserialize, Serialize};

use super::error::ModelError;
use super::validation::Validate;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Version(semver_crate::Version);

impl Version {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self(semver_crate::Version::new(major, minor, patch))
    }

    pub fn parse(value: &str) -> Result<Self, ModelError> {
        value.parse()
    }

    /// Parses a version string, accepting common Winget-style variants.
    pub fn parse_lossy(value: &str) -> Result<Self, ModelError> {
        match Self::parse(value) {
            Ok(version) => Ok(version),
            Err(strict_err) => {
                let normalized = match normalize_lossy_version(value) {
                    Some(normalized) => normalized,
                    None => return Err(strict_err),
                };

                semver_crate::Version::parse(&normalized)
                    .map(Self)
                    .map_err(|err| {
                        ModelError::invalid_version(
                            value,
                            format!(
                                "{strict_err}; normalized to {normalized}, but parsing still failed: {err}"
                            ),
                        )
                    })
            }
        }
    }

    pub fn as_semver(&self) -> &semver_crate::Version {
        &self.0
    }
}

impl FromStr for Version {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        semver_crate::Version::parse(s)
            .map(Self)
            .map_err(|err| ModelError::invalid_version(s, err.to_string()))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<semver_crate::Version> for Version {
    fn from(value: semver_crate::Version) -> Self {
        Self(value)
    }
}

impl From<Version> for semver_crate::Version {
    fn from(value: Version) -> Self {
        value.0
    }
}

impl From<Version> for String {
    fn from(value: Version) -> Self {
        value.to_string()
    }
}

fn normalize_lossy_version(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let trimmed = strip_version_prefix(trimmed);
    let tokens = tokenize_version(trimmed);
    if tokens.is_empty() {
        return None;
    }

    if tokens.first().is_none_or(|token| !starts_with_digit(token)) {
        return Some(format!("0.0.0-{}", join_identifiers(tokens.iter())));
    }

    let mut core = Vec::with_capacity(3);
    let mut extra = Vec::new();
    let mut has_non_numeric_extra = false;

    for token in tokens {
        if token.is_empty() {
            continue;
        }

        if core.len() < 3 {
            if token.chars().all(|ch| ch.is_ascii_digit()) {
                core.push(normalize_numeric_identifier(token));
                continue;
            }

            if let Some((digits, suffix)) = split_numeric_prefix(token) {
                core.push(normalize_numeric_identifier(digits));
                if !suffix.is_empty() {
                    extra.push(suffix.to_string());
                    has_non_numeric_extra = true;
                }
                continue;
            }

            extra.push(token.to_string());
            has_non_numeric_extra = true;
            continue;
        }

        if token.chars().all(|ch| ch.is_ascii_digit()) {
            extra.push(normalize_numeric_identifier(token));
        } else {
            extra.push(token.to_string());
            has_non_numeric_extra = true;
        }
    }

    while core.len() < 3 {
        core.push(String::from("0"));
    }

    let mut normalized = core.join(".");
    if !extra.is_empty() {
        normalized.push(if has_non_numeric_extra { '-' } else { '+' });
        normalized.push_str(&extra.join("."));
    }

    Some(normalized)
}

fn strip_version_prefix(value: &str) -> &str {
    if let Some(stripped) = value
        .strip_prefix('v')
        .or_else(|| value.strip_prefix('V'))
        .filter(|rest| rest.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
    {
        stripped
    } else {
        value
    }
}

fn tokenize_version(value: &str) -> Vec<&str> {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect()
}

fn join_identifiers<'a, I>(tokens: I) -> String
where
    I: IntoIterator<Item = &'a &'a str>,
{
    tokens
        .into_iter()
        .map(|token| {
            if token.chars().all(|ch| ch.is_ascii_digit()) {
                normalize_numeric_identifier(token)
            } else {
                (*token).to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn normalize_numeric_identifier(value: &str) -> String {
    let trimmed = value.trim_start_matches('0');
    if trimmed.is_empty() {
        String::from("0")
    } else {
        trimmed.to_string()
    }
}

fn split_numeric_prefix(value: &str) -> Option<(&str, &str)> {
    let digits = value
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .count();

    if digits == 0 {
        return None;
    }

    Some(value.split_at(digits))
}

fn starts_with_digit(value: &str) -> bool {
    value.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

impl Validate for Version {
    fn validate(&self) -> Result<(), ModelError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Version;

    #[test]
    fn parses_semver_and_orders_versions() {
        let version = Version::parse("1.2.3").expect("version should parse");
        let newer = Version::parse("1.2.4").expect("version should parse");

        assert!(newer > version);
        assert_eq!(version.to_string(), "1.2.3");
    }

    #[test]
    fn parses_common_winget_versions_lossily() {
        let cases = [
            ("v2.6.0", "2.6.0"),
            ("2026.03.17", "2026.3.17"),
            ("4.0", "4.0.0"),
            ("115.0.5790.136", "115.0.5790+136"),
            ("20240608.083822.1ed9031", "20240608.83822.1-ed9031"),
            (
                "N-123778-g3b55818764-20260331",
                "0.0.0-N.123778.g3b55818764.20260331",
            ),
        ];

        for (input, expected) in cases {
            let parsed = Version::parse_lossy(input).expect("version should parse lossy");
            assert_eq!(parsed.to_string(), expected);
        }
    }
}
