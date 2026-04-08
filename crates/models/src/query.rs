#[derive(Debug, Clone)]
pub struct PackageQuery {
    pub terms: Vec<String>,
    pub version: Option<String>,
}

impl PackageQuery {
    pub fn text(&self) -> String {
        self.terms.join(" ")
    }
}
