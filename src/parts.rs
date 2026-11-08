// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{Boundary, MultipartError, MultipartEvent, MultipartLimits, MultipartParser, PartHead};
use std::fmt;
use std::iter::FusedIterator;

/// One part of a complete body: its header section and content, borrowed from the body.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Part<'a> {
    head: PartHead<'a>,
    content: &'a [u8],
}

impl<'a> Part<'a> {
    /// The header section.
    pub fn head(&self) -> PartHead<'a> {
        self.head
    }

    /// The content.
    pub fn content(&self) -> &'a [u8] {
        self.content
    }
}

/// Shows the length of the content rather than its bytes, since content must not reach logs.
impl fmt::Debug for Part<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Part")
            .field("head", &self.head)
            .field("content", &format_args!("<{} bytes>", self.content.len()))
            .finish()
    }
}

/// The parts of a complete body held in memory, in order, as [`parts`] produces them.
#[derive(Clone)]
pub struct Parts<'a> {
    parser: MultipartParser,
    body: &'a [u8],
    position: usize,
    failed: bool,
}

/// The parts of a complete body, with the default limits.
///
/// ```
/// use base_web_multipart::{Boundary, ParameterRules, PartHeaders, parts};
/// use base_message::FieldSection;
///
/// let body = b"--b\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\none\r\n--b\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\ntwo\r\n--b--";
/// let boundary = Boundary::new("b")?;
/// let mut fields = Vec::new();
/// for part in parts(body, &boundary) {
///     let part = part?;
///     let headers = PartHeaders::interpret(part.head())?;
///     let disposition = headers.content_disposition.ok_or("a form part has a disposition")?;
///     let name = disposition.name(ParameterRules::FormData)?.ok_or("a form part has a name")?.into_owned();
///     fields.push((name, part.content()));
/// }
/// assert_eq!(fields, [("a".to_owned(), &b"one"[..]), ("b".to_owned(), &b"two"[..])]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parts<'a>(body: &'a [u8], boundary: &Boundary) -> Parts<'a> {
    Parts::with_limits(body, boundary, MultipartLimits::default())
}

impl<'a> Parts<'a> {
    /// The parts of a complete body, with these limits.
    pub fn with_limits(body: &'a [u8], boundary: &Boundary, limits: MultipartLimits) -> Self {
        Self {
            parser: MultipartParser::with_limits(boundary, limits),
            body,
            position: 0,
            failed: false,
        }
    }

    fn next_part(&mut self) -> Result<Option<Part<'a>>, MultipartError> {
        let body = self.body;
        let (mut head, mut content, mut start) = (None, self.position..self.position, None);
        loop {
            let parsed = self.parser.parse(&body[self.position..])?;
            self.position += parsed.consumed;
            match parsed.event {
                MultipartEvent::Part(part) => {
                    head = Some(part);
                    content = self.position..self.position;
                    start = None;
                }
                // Each piece ends where the parser stopped, and pieces of one part's content are adjacent in the body, so the content runs from the first piece's start to the last piece's end.
                MultipartEvent::Content(piece) => {
                    content = *start.get_or_insert(self.position - piece.len())..self.position
                }
                MultipartEvent::PartEnd => {
                    return Ok(head.map(|head| Part {
                        head,
                        content: &body[content],
                    }));
                }
                MultipartEvent::End => return Ok(None),
                // The whole body was given, so more input will not arrive.
                MultipartEvent::Incomplete => return self.parser.finish().and(Ok(None)),
            }
        }
    }
}

/// Shows the position in the body and its length, since the body must not reach logs.
impl fmt::Debug for Parts<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Parts")
            .field("position", &self.position)
            .field("length", &self.body.len())
            .finish()
    }
}

impl<'a> Iterator for Parts<'a> {
    type Item = Result<Part<'a>, MultipartError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let result = self.next_part().transpose();
        self.failed = !matches!(result, Some(Ok(_)));
        result
    }
}

impl FusedIterator for Parts<'_> {}
