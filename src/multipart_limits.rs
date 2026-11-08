// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

/// The structural limits a parser applies, so that a hostile body cannot make it buffer or count without bound.
///
/// Content is not limited here: a part's content passes through as it arrives, and limits on content, such as for form fields and uploads, belong to its consumer.
///
/// ```
/// use base_web_multipart::MultipartLimits;
///
/// let limits = MultipartLimits { part_count_limit: 16, ..MultipartLimits::default() };
/// assert_eq!(limits.header_size_limit, MultipartLimits::HEADER_SIZE_LIMIT);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MultipartLimits {
    /// The bytes before the first boundary, which are ignored.
    pub preamble_size_limit: usize,
    /// The bytes of one part's header section, including its line breaks, and of any padding after the boundary before it. Padding after the closing boundary is limited too.
    pub header_size_limit: usize,
    /// The fields in one part's header section.
    pub header_count_limit: usize,
    /// The parts in the body.
    pub part_count_limit: usize,
}

impl MultipartLimits {
    /// The default preamble size limit: 64 KiB.
    pub const PREAMBLE_SIZE_LIMIT: usize = 64 * 1024;
    /// The default header size limit: 64 KiB.
    pub const HEADER_SIZE_LIMIT: usize = 64 * 1024;
    /// The default header count limit.
    pub const HEADER_COUNT_LIMIT: usize = 64;
    /// The default part count limit.
    pub const PART_COUNT_LIMIT: usize = 128;
}

impl Default for MultipartLimits {
    fn default() -> Self {
        Self {
            preamble_size_limit: Self::PREAMBLE_SIZE_LIMIT,
            header_size_limit: Self::HEADER_SIZE_LIMIT,
            header_count_limit: Self::HEADER_COUNT_LIMIT,
            part_count_limit: Self::PART_COUNT_LIMIT,
        }
    }
}
