// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::PartHead;
use std::fmt;

/// What one call to [`MultipartParser::parse`](crate::MultipartParser::parse) found, with the number of input bytes it consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Parsed<'a> {
    /// The input bytes consumed. The next call's input must begin with the bytes after them.
    pub consumed: usize,
    /// What the consumed bytes held.
    pub event: MultipartEvent<'a>,
}

/// One step through a multipart body, borrowed from the parser's input.
///
/// A body produces, for each part, `Part`, any number of `Content`, then `PartEnd`, and finally `End`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MultipartEvent<'a> {
    /// A part begins, with its header section.
    Part(PartHead<'a>),
    /// Some of the current part's content. A part's content may arrive in any number of pieces.
    Content(&'a [u8]),
    /// The current part's content is complete.
    PartEnd,
    /// The closing boundary was reached. Anything after it, the epilogue, is ignored.
    End,
    /// The input ends before the next event: call again with more input, beginning with the bytes not yet consumed.
    Incomplete,
}

/// Shows the length of content rather than its bytes, since content such as a password field must not reach logs.
impl fmt::Debug for MultipartEvent<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Part(head) => formatter.debug_tuple("Part").field(head).finish(),
            Self::Content(content) => write!(formatter, "Content(<{} bytes>)", content.len()),
            Self::PartEnd => formatter.write_str("PartEnd"),
            Self::End => formatter.write_str("End"),
            Self::Incomplete => formatter.write_str("Incomplete"),
        }
    }
}
