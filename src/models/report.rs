#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReport {
    pub sections: Vec<ReportSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSection {
    pub title: String,
    pub entries: Vec<(String, String)>,
}

impl RuntimeReport {
    pub fn new(sections: Vec<ReportSection>) -> Self {
        Self { sections }
    }
}
