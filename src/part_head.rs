// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::fmt;
use std::iter::FusedIterator;

/// A part's header section, borrowed from the parser's input and already checked: each line is a field name, a colon, and a value, ending with CRLF.
///
/// Interpret it with a [`base_message::FieldSection`], such as [`PartHeaders`](crate::PartHeaders), from [`Self::fields`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartHead<'a> {
    /// The header lines, each ending with CRLF, without the empty line that ends the section.
    section: &'a [u8],
}

impl<'a> PartHead<'a> {
    pub(crate) fn new(section: &'a [u8]) -> Self {
        Self { section }
    }

    /// The fields, in order, as name and value pairs, with spaces and tabs around each value removed.
    pub fn fields(&self) -> PartFields<'a> {
        PartFields { rest: self.section }
    }

    /// The number of fields, counted by reading the section.
    pub fn len(&self) -> usize {
        self.fields().count()
    }

    /// Whether the part has no fields.
    pub fn is_empty(&self) -> bool {
        self.section.is_empty()
    }

    /// The header section as it was written, without the empty line that ends it.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.section
    }
}

/// Shows field names and the lengths of their values, since values can carry credentials that must not reach logs.
impl fmt::Debug for PartHead<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fields = self
            .fields()
            .map(|(name, value)| (name, format!("<{} bytes>", value.len())));
        formatter
            .debug_struct("PartHead")
            .field("fields", &fields.collect::<Vec<_>>())
            .finish()
    }
}

impl<'a> IntoIterator for PartHead<'a> {
    type Item = (&'a str, &'a [u8]);
    type IntoIter = PartFields<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields()
    }
}

/// The fields of a [`PartHead`].
#[derive(Clone)]
pub struct PartFields<'a> {
    rest: &'a [u8],
}

/// Shows the length of the remaining header section, since field values must not reach logs.
impl fmt::Debug for PartFields<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PartFields")
            .field("remaining", &format_args!("<{} bytes>", self.rest.len()))
            .finish()
    }
}

impl<'a> Iterator for PartFields<'a> {
    type Item = (&'a str, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        // The parser checked every line, so each has a colon and ends with CRLF, and its name is ASCII.
        let end = memchr::memmem::find(self.rest, b"\r\n")?;
        let line = &self.rest[..end];
        self.rest = &self.rest[end + 2..];
        let colon = memchr::memchr(b':', line).expect("a checked header line has a colon");
        let name = std::str::from_utf8(&line[..colon]).expect("a checked field name is ASCII");
        Some((name, base_message::trim_whitespace(&line[colon + 1..])))
    }
}

impl FusedIterator for PartFields<'_> {}
