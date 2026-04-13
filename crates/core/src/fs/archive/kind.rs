/// Archive formats supported by WinBrew's extraction layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    /// ZIP archive format.
    Zip,
    /// 7-Zip archive format.
    SevenZip,
    /// Tar-based archive family, including `.tar`, `.tar.gz`, `.tgz`, and `.tbz2`.
    Tar,
    /// RAR archive format.
    Rar,
}

impl ArchiveKind {
    /// Return the canonical lowercase name used in errors and logs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::SevenZip => "7z",
            Self::Tar => "tar",
            Self::Rar => "rar",
        }
    }
}
