#[derive(Clone, Copy, Debug)]
pub(crate) struct ExtractionLimits {
    pub(crate) max_total_size: u64,
    pub(crate) max_file_count: usize,
    pub(crate) max_compression_ratio: u64,
    pub(crate) max_path_depth: usize,
}

impl Default for ExtractionLimits {
    fn default() -> Self {
        Self {
            max_total_size: 10 * 1024 * 1024 * 1024,
            max_file_count: 100_000,
            max_compression_ratio: 100,
            max_path_depth: 255,
        }
    }
}
