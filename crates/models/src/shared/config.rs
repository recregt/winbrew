#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSection {
    pub title: String,
    pub entries: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValue {
    pub value: String,
    pub source: ConfigValueSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigValueSource {
    Env,
    File,
}
