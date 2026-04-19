/// Archive formats supported by WinBrew's extraction layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    /// ZIP archive format.
    Zip,
    /// 7-Zip archive format.
    SevenZip,
    /// GZip single-file compression format.
    Gzip,
    /// Tar-based archive family, including `.tar`, `.tar.gz`, `.tgz`, `.tbz2`, and `.tar.bz2`.
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
            Self::Gzip => "gzip",
            Self::Tar => "tar",
            Self::Rar => "rar",
        }
    }
}
