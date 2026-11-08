// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::fmt;

/// Why a multipart body could not be parsed, with the offset in the body where the parser found it. It never includes the body's bytes, which may be large or untrusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MultipartError {
    kind: MultipartErrorKind,
    offset: u64,
}

/// The kind of [`MultipartError`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MultipartErrorKind {
    /// The preamble is longer than its limit.
    PreambleSizeLimit,
    /// A part's header section is longer than its limit.
    HeaderSizeLimit,
    /// A part's header section has more fields than its limit.
    HeaderCountLimit,
    /// The body has more parts than its limit.
    PartCountLimit,
    /// A header line is not `name: value` with a valid name and value, or does not end with CRLF.
    InvalidField,
    /// A header line is folded, continuing the previous line, which this parser does not accept.
    FoldedField,
    /// A boundary is followed by something other than optional spaces and tabs and a line break, or `--`, so it appears in content where RFC 2046 does not allow it.
    InvalidDelimiter,
    /// The body ended before its closing boundary.
    UnexpectedEnd,
}

impl MultipartError {
    pub(crate) fn new(kind: MultipartErrorKind, offset: u64) -> Self {
        Self { kind, offset }
    }

    /// What went wrong.
    pub fn kind(&self) -> MultipartErrorKind {
        self.kind
    }

    /// The offset in the body, in bytes, where the parser found the problem.
    pub fn offset(&self) -> u64 {
        self.offset
    }
}

impl fmt::Display for MultipartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let description = match self.kind {
            MultipartErrorKind::PreambleSizeLimit => "multipart preamble exceeds its size limit",
            MultipartErrorKind::HeaderSizeLimit => {
                "multipart part header section exceeds its size limit"
            }
            MultipartErrorKind::HeaderCountLimit => {
                "multipart part header section exceeds its field count limit"
            }
            MultipartErrorKind::PartCountLimit => "multipart body exceeds its part count limit",
            MultipartErrorKind::InvalidField => "invalid multipart part header field",
            MultipartErrorKind::FoldedField => "folded multipart part header field",
            MultipartErrorKind::InvalidDelimiter => "invalid multipart boundary line",
            MultipartErrorKind::UnexpectedEnd => "multipart body ends before its closing boundary",
        };
        write!(formatter, "{description} at byte {}", self.offset)
    }
}

impl std::error::Error for MultipartError {}
