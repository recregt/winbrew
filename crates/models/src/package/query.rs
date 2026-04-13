//! Lightweight query model for package search.

/// A normalized package search query.
///
/// `terms` preserves the tokenized search text while `version` carries an
/// optional version filter that can be applied by the search layer.
#[derive(Debug, Clone)]
pub struct PackageQuery {
    /// Search terms in display order.
    pub terms: Vec<String>,
    /// Optional version constraint supplied by the caller.
    pub version: Option<String>,
}

impl PackageQuery {
    /// Reconstruct the human-readable search text from the stored terms.
    pub fn text(&self) -> String {
        self.terms.join(" ")
    }
}
