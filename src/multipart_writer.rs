// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::Boundary;
use crate::syntax::{is_field_name_byte, is_field_value_byte};
use memchr::memmem::Finder;
use std::fmt;

/// A sans-I/O writer for a multipart body, which appends the body's bytes to a caller's buffer, to be sent as the caller chooses.
///
/// It refuses whatever would change the body's structure for a reader: a field name or value that is not valid, which could inject header lines, and a header line or content that contains the boundary after a line break, which would end the part early. That includes the boundary after a bare CR or LF, which some readers also take for a delimiter. A [generated](Boundary::generate) boundary makes content containing it practically impossible, but the check also covers boundaries chosen by the caller.
///
/// The writer does not apply a reader's limits: a body with more parts or header fields than a reader allows, such as [`MultipartLimits`](crate::MultipartLimits)'s defaults, is written, and that reader refuses it.
///
/// ```
/// use base_web_multipart::{Boundary, ContentDisposition, MultipartWriter};
///
/// let mut writer = MultipartWriter::new(Boundary::new("b")?);
/// let mut body = Vec::new();
/// let disposition = ContentDisposition::form_data("note")?.to_string();
/// writer.start_part([("Content-Disposition", disposition.as_bytes())], &mut body)?;
/// writer.write_content(b"hello", &mut body)?;
/// writer.finish(&mut body);
/// assert_eq!(body, b"--b\r\nContent-Disposition: form-data; name=\"note\"\r\n\r\nhello\r\n--b--\r\n");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct MultipartWriter {
    boundary: Boundary,
    /// `CRLF "--" boundary`, which ends a part's content.
    delimiter: Vec<u8>,
    /// Finds `"--" boundary`, so that what precedes it can be checked.
    finder: Finder<'static>,
    state: State,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// No part has started.
    Start,
    /// In a part's content. When its last bytes are a line break and the start of `"--" boundary`, the bytes of `"--" boundary` they match, which the next content must not complete.
    Content { matched: Option<usize> },
}

/// Why a writer refused to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MultipartWriteError {
    /// A field name is empty or has a character other than visible ASCII except the colon.
    InvalidFieldName,
    /// A field value has a control character other than horizontal tab, such as CR or LF.
    InvalidFieldValue,
    /// A header line or content contains the boundary after a line break, so a reader would see the part end there.
    Delimiter,
    /// Content was written before any part started.
    NoPart,
}

impl MultipartWriter {
    /// A writer for a body with this boundary.
    pub fn new(boundary: Boundary) -> Self {
        let delimiter = [b"\r\n--", boundary.as_str().as_bytes()].concat();
        let finder = Finder::new(&delimiter[2..]).into_owned();
        Self {
            boundary,
            delimiter,
            finder,
            state: State::Start,
        }
    }

    /// The boundary, for the body's media type, such as from [`Boundary::form_data_media_type`].
    pub fn boundary(&self) -> &Boundary {
        &self.boundary
    }

    /// End any current part and begin another, with these header fields, in order.
    ///
    /// # Errors
    ///
    /// When a field name or value is invalid, or a header line begins with the boundary delimiter. Nothing is written, and the writer is unchanged.
    pub fn start_part<N, V>(
        &mut self,
        fields: impl IntoIterator<Item = (N, V)>,
        output: &mut Vec<u8>,
    ) -> Result<(), MultipartWriteError>
    where
        N: AsRef<str>,
        V: AsRef<[u8]>,
    {
        let length = output.len();
        let result = self.write_head(fields, output);
        if result.is_err() {
            output.truncate(length);
        }
        result
    }

    fn write_head<N, V>(
        &mut self,
        fields: impl IntoIterator<Item = (N, V)>,
        output: &mut Vec<u8>,
    ) -> Result<(), MultipartWriteError>
    where
        N: AsRef<str>,
        V: AsRef<[u8]>,
    {
        let delimiter = if self.state == State::Start {
            &self.delimiter[2..]
        } else {
            &self.delimiter[..]
        };
        output.extend_from_slice(delimiter);
        output.extend_from_slice(b"\r\n");
        for (name, value) in fields {
            let (name, value) = (name.as_ref().as_bytes(), value.as_ref());
            if name.is_empty() || !name.iter().all(|&byte| is_field_name_byte(byte)) {
                return Err(MultipartWriteError::InvalidFieldName);
            }
            if !value.iter().all(|&byte| is_field_value_byte(byte)) {
                return Err(MultipartWriteError::InvalidFieldValue);
            }
            let line = output.len();
            output.extend_from_slice(name);
            output.extend_from_slice(b": ");
            output.extend_from_slice(value);
            // Each line follows a line break, so a line beginning with the boundary would form the delimiter.
            if output[line..].starts_with(&self.delimiter[2..]) {
                return Err(MultipartWriteError::Delimiter);
            }
            output.extend_from_slice(b"\r\n");
        }
        output.extend_from_slice(b"\r\n");
        // The content follows the line break that ends the header section.
        self.state = State::Content { matched: Some(0) };
        Ok(())
    }

    /// Append some of the current part's content.
    ///
    /// # Errors
    ///
    /// When no part has started, or the content, with the content before it, contains the boundary after a line break. Nothing is written, and the writer is unchanged.
    pub fn write_content(
        &mut self,
        content: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<(), MultipartWriteError> {
        let State::Content { matched } = self.state else {
            return Err(MultipartWriteError::NoPart);
        };
        let matched = self.scan(matched, content)?;
        output.extend_from_slice(content);
        self.state = State::Content { matched };
        Ok(())
    }

    /// End the body with its closing boundary. A body with no parts is the closing boundary alone.
    pub fn finish(self, output: &mut Vec<u8>) {
        let delimiter = if self.state == State::Start {
            &self.delimiter[2..]
        } else {
            &self.delimiter[..]
        };
        output.extend_from_slice(delimiter);
        output.extend_from_slice(b"--\r\n");
    }

    /// The bytes of `"--" boundary` that follow the last line break at the end of the content, given those the content before it matched, or an error when the content completes `"--" boundary` after a line break. `"--" boundary` contains no line break, so a match can only begin right after one.
    fn scan(
        &self,
        matched: Option<usize>,
        content: &[u8],
    ) -> Result<Option<usize>, MultipartWriteError> {
        let dash_boundary = &self.delimiter[2..];
        let continues = |matched: usize, after: &[u8]| {
            let remaining = &dash_boundary[matched..];
            let length = remaining.len().min(after.len());
            (after[..length] == remaining[..length]).then_some(matched + length)
        };
        if let Some(matched) = matched.and_then(|matched| continues(matched, content)) {
            return if matched == dash_boundary.len() {
                Err(MultipartWriteError::Delimiter)
            } else {
                Ok(Some(matched))
            };
        }
        if self
            .finder
            .find_iter(content)
            .any(|index| index > 0 && matches!(content[index - 1], b'\r' | b'\n'))
        {
            return Err(MultipartWriteError::Delimiter);
        }
        let start = content.len().saturating_sub(dash_boundary.len());
        Ok(memchr::memchr2_iter(b'\r', b'\n', &content[start..])
            .find_map(|index| continues(0, &content[start + index + 1..])))
    }
}

impl fmt::Display for MultipartWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidFieldName => "invalid multipart part header field name",
            Self::InvalidFieldValue => "invalid multipart part header field value",
            Self::Delimiter => "multipart part contains its boundary delimiter",
            Self::NoPart => "multipart content written before any part",
        })
    }
}

impl std::error::Error for MultipartWriteError {}
