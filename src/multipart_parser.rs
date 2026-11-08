// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::syntax::{is_field_name_byte, is_field_value_byte};
use crate::{
    Boundary, MultipartError, MultipartErrorKind, MultipartEvent, MultipartLimits, Parsed, PartHead,
};
use memchr::memmem::Finder;

/// A sans-I/O parser for a multipart body, as RFC 2046 section 5.1 defines it, fed bytes as they arrive.
///
/// The caller owns the buffer. Each call to [`Self::parse`] reports how many bytes it consumed and what they held; the caller drops the consumed bytes, appends whatever arrives next, and calls again with what remains. Events borrow from the input, so content and header fields are never copied. A part's header section is returned whole, so the caller must be able to buffer [`Self::minimum_buffer_size`] bytes; the parser reports an error rather than needing more.
///
/// ```
/// use base_web_multipart::{Boundary, MultipartEvent, MultipartParser};
///
/// let body = b"--b\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nvalue\r\n--b--\r\n";
/// let mut parser = MultipartParser::new(&Boundary::new("b")?);
/// let mut input = &body[..];
/// let mut content = Vec::new();
/// loop {
///     let parsed = parser.parse(input)?;
///     match parsed.event {
///         MultipartEvent::Part(head) => assert_eq!(head.len(), 1),
///         MultipartEvent::Content(bytes) => content.extend_from_slice(bytes),
///         MultipartEvent::PartEnd => {}
///         MultipartEvent::End => break,
///         MultipartEvent::Incomplete => unreachable!("the whole body is available"),
///     }
///     input = &input[parsed.consumed..];
/// }
/// assert_eq!(content, b"value");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct MultipartParser {
    /// `CRLF "--" boundary`, which ends a part's content.
    delimiter: Vec<u8>,
    /// Finds `"--" boundary`, wherever it appears, so that what precedes it can be checked.
    finder: Finder<'static>,
    limits: MultipartLimits,
    state: State,
    /// The bytes consumed before the current call, for the offsets of errors.
    offset: u64,
    preamble_size: usize,
    part_count: usize,
    /// The error the parser reported, which it reports again for every later call.
    failed: Option<MultipartError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Nothing consumed: the body may begin with the boundary itself, without a preceding line break.
    Start,
    /// Before the first boundary.
    Preamble,
    /// After a boundary, before `--` or the line break that begins a part, with the spaces and tabs consumed so far.
    Delimiter {
        padding: usize,
    },
    /// After `--` that closes the body, with the spaces and tabs consumed so far, before the line break that ends the closing boundary.
    Closing {
        padding: usize,
    },
    /// In a part's header section, with the padding after its boundary, which counts toward its size, and the bytes of the input already searched for its end.
    Headers {
        padding: usize,
        searched: usize,
    },
    /// At the empty line that ends a part's header section, whose line break may also begin the delimiter of a part with no content.
    BodyStart,
    Content,
    End,
}

/// A `"--" boundary` in the input, classified by what precedes it.
enum Found {
    /// Preceded by CRLF: a delimiter, starting at the CR.
    Delimiter(usize),
    /// Preceded by a bare CR or LF, at this position, which some readers take for a delimiter.
    Bare(usize),
    /// Not in the input.
    Absent,
}

impl MultipartParser {
    /// A parser for a body with this boundary, with the default limits.
    pub fn new(boundary: &Boundary) -> Self {
        Self::with_limits(boundary, MultipartLimits::default())
    }

    /// A parser for a body with this boundary and these limits.
    pub fn with_limits(boundary: &Boundary, limits: MultipartLimits) -> Self {
        let delimiter = [b"\r\n--", boundary.as_str().as_bytes()].concat();
        let finder = Finder::new(&delimiter[2..]).into_owned();
        Self {
            delimiter,
            finder,
            limits,
            state: State::Start,
            offset: 0,
            preamble_size: 0,
            part_count: 0,
            failed: None,
        }
    }

    /// The limits this parser applies.
    pub fn limits(&self) -> MultipartLimits {
        self.limits
    }

    /// The input a caller must be able to buffer: given at least this many bytes, [`Self::parse`] always consumes some or reports an event other than [`MultipartEvent::Incomplete`].
    pub fn minimum_buffer_size(&self) -> usize {
        self.limits
            .header_size_limit
            .saturating_add(2)
            .max(self.delimiter.len())
    }

    /// Whether the closing boundary line has been read, so that [`MultipartEvent::End`] was reported.
    pub fn is_finished(&self) -> bool {
        self.state == State::End
    }

    /// Parse the next event from `input`, which must begin with the bytes the previous call did not consume.
    ///
    /// # Errors
    ///
    /// A [`MultipartError`] when the body is malformed or exceeds a limit. Once the parser reports an error, it reports the same error for every later call.
    pub fn parse<'a>(&mut self, input: &'a [u8]) -> Result<Parsed<'a>, MultipartError> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        let mut consumed = 0;
        match self.step(input, &mut consumed) {
            Ok(event) => {
                self.offset += consumed as u64;
                Ok(Parsed { consumed, event })
            }
            Err(kind) => {
                let error = MultipartError::new(kind, self.offset + consumed as u64);
                self.failed = Some(error);
                Err(error)
            }
        }
    }

    /// Check that the body was complete, once its input has ended. A body may end immediately after its closing `--`, without a line break.
    ///
    /// # Errors
    ///
    /// The parser's error, if it reported one, or [`MultipartErrorKind::UnexpectedEnd`] when the closing boundary was not reached.
    pub fn finish(&self) -> Result<(), MultipartError> {
        if let Some(error) = self.failed {
            return Err(error);
        }
        match self.state {
            State::End | State::Closing { .. } => Ok(()),
            _ => Err(MultipartError::new(
                MultipartErrorKind::UnexpectedEnd,
                self.offset,
            )),
        }
    }

    fn step<'a>(
        &mut self,
        input: &'a [u8],
        consumed: &mut usize,
    ) -> Result<MultipartEvent<'a>, MultipartErrorKind> {
        loop {
            let rest = &input[*consumed..];
            match self.state {
                State::Start => {
                    let dash_boundary = &self.delimiter[2..];
                    if rest.starts_with(dash_boundary) {
                        *consumed += dash_boundary.len();
                        self.state = State::Delimiter { padding: 0 };
                    } else if rest.len() < dash_boundary.len() && dash_boundary.starts_with(rest) {
                        return Ok(MultipartEvent::Incomplete);
                    } else {
                        self.state = State::Preamble;
                    }
                }
                State::Preamble => {
                    let (skipped, found) = match self.find(rest) {
                        Found::Delimiter(index) => (index, true),
                        Found::Bare(index) => {
                            *consumed += index;
                            return Err(MultipartErrorKind::InvalidDelimiter);
                        }
                        Found::Absent => (self.safe_length(rest), false),
                    };
                    self.preamble_size = self.preamble_size.saturating_add(skipped);
                    if self.preamble_size > self.limits.preamble_size_limit {
                        return Err(MultipartErrorKind::PreambleSizeLimit);
                    }
                    *consumed += skipped;
                    if !found {
                        return Ok(MultipartEvent::Incomplete);
                    }
                    *consumed += self.delimiter.len();
                    self.state = State::Delimiter { padding: 0 };
                }
                State::Delimiter { padding } => {
                    if padding == 0 && rest.starts_with(b"--") {
                        *consumed += 2;
                        self.state = State::Closing { padding: 0 };
                        continue;
                    }
                    let padding = self.pad(rest, padding, consumed)?;
                    self.state = State::Delimiter { padding };
                    let rest = &input[*consumed..];
                    if rest.starts_with(b"\r\n") {
                        self.part_count += 1;
                        if self.part_count > self.limits.part_count_limit {
                            return Err(MultipartErrorKind::PartCountLimit);
                        }
                        *consumed += 2;
                        self.state = State::Headers {
                            padding,
                            searched: 0,
                        };
                    } else if rest.is_empty() || rest == b"\r" || (padding == 0 && rest == b"-") {
                        return Ok(MultipartEvent::Incomplete);
                    } else {
                        return Err(MultipartErrorKind::InvalidDelimiter);
                    }
                }
                State::Closing { padding } => {
                    let padding = self.pad(rest, padding, consumed)?;
                    self.state = State::Closing { padding };
                    let rest = &input[*consumed..];
                    if rest.starts_with(b"\r\n") {
                        *consumed += 2;
                        self.state = State::End;
                        return Ok(MultipartEvent::End);
                    } else if rest.is_empty() || rest == b"\r" {
                        // The body may also end here, which `finish` accepts.
                        return Ok(MultipartEvent::Incomplete);
                    } else {
                        return Err(MultipartErrorKind::InvalidDelimiter);
                    }
                }
                State::Headers { padding, searched } => {
                    // The padding after the boundary counts toward the header section's size.
                    let limit = self.limits.header_size_limit - padding;
                    let length = if rest.starts_with(b"\r\n") {
                        Some(0)
                    } else {
                        let start = searched.saturating_sub(3).min(rest.len());
                        memchr::memmem::find(&rest[start..], b"\r\n\r\n")
                            .map(|index| start + index + 2)
                    };
                    let Some(length) = length else {
                        // A section within the limit ends within its length and the empty line's CRLF.
                        if rest.len() >= limit.saturating_add(2) {
                            return Err(MultipartErrorKind::HeaderSizeLimit);
                        }
                        self.state = State::Headers {
                            padding,
                            searched: rest.len(),
                        };
                        return Ok(MultipartEvent::Incomplete);
                    };
                    if length > limit {
                        return Err(MultipartErrorKind::HeaderSizeLimit);
                    }
                    let section = &rest[..length];
                    self.check_section(section)?;
                    *consumed += length;
                    self.state = State::BodyStart;
                    return Ok(MultipartEvent::Part(PartHead::new(section)));
                }
                State::BodyStart => {
                    // RFC 2046 makes the content `[CRLF *OCTET]`, so the empty line's CRLF is also the delimiter's when the part has no content.
                    if rest.starts_with(&self.delimiter) {
                        *consumed += self.delimiter.len();
                        self.state = State::Delimiter { padding: 0 };
                        return Ok(MultipartEvent::PartEnd);
                    }
                    if rest.len() < self.delimiter.len() && self.delimiter.starts_with(rest) {
                        return Ok(MultipartEvent::Incomplete);
                    }
                    *consumed += 2;
                    self.state = State::Content;
                }
                State::Content => {
                    return match self.find(rest) {
                        Found::Delimiter(0) => {
                            *consumed += self.delimiter.len();
                            self.state = State::Delimiter { padding: 0 };
                            Ok(MultipartEvent::PartEnd)
                        }
                        Found::Delimiter(index) => {
                            *consumed += index;
                            Ok(MultipartEvent::Content(&rest[..index]))
                        }
                        Found::Bare(index) => {
                            *consumed += index;
                            Err(MultipartErrorKind::InvalidDelimiter)
                        }
                        Found::Absent => match self.safe_length(rest) {
                            0 => Ok(MultipartEvent::Incomplete),
                            length => {
                                *consumed += length;
                                Ok(MultipartEvent::Content(&rest[..length]))
                            }
                        },
                    };
                }
                State::End => {
                    // The epilogue is ignored.
                    *consumed += rest.len();
                    return Ok(MultipartEvent::End);
                }
            }
        }
    }

    /// Consume the spaces and tabs of transport padding after a boundary, which count toward the header size limit.
    fn pad(
        &self,
        rest: &[u8],
        padding: usize,
        consumed: &mut usize,
    ) -> Result<usize, MultipartErrorKind> {
        let spaces = rest
            .iter()
            .take_while(|&&byte| byte == b' ' || byte == b'\t')
            .count();
        let padding = padding + spaces;
        if padding > self.limits.header_size_limit {
            return Err(MultipartErrorKind::HeaderSizeLimit);
        }
        *consumed += spaces;
        Ok(padding)
    }

    /// The first `"--" boundary` in `input` that follows a line break. One after CRLF is a delimiter; one after a bare CR or LF is an error, since some readers take it for a delimiter; others are content.
    fn find(&self, input: &[u8]) -> Found {
        let mut from = 0;
        while let Some(index) = self.finder.find(&input[from..]).map(|index| from + index) {
            if index >= 2 && &input[index - 2..index] == b"\r\n" {
                return Found::Delimiter(index - 2);
            }
            if index >= 1 && matches!(input[index - 1], b'\r' | b'\n') {
                return Found::Bare(index - 1);
            }
            from = index + 1;
        }
        Found::Absent
    }

    /// The bytes at the start of `input` that cannot be part of a boundary still arriving: everything before the first line break from which the rest could become `CRLF "--" boundary`, or a boundary after a bare line break.
    fn safe_length(&self, input: &[u8]) -> usize {
        let dash_boundary = &self.delimiter[2..];
        let start = input.len().saturating_sub(self.delimiter.len() - 1);
        memchr::memchr2_iter(b'\r', b'\n', &input[start..])
            .map(|index| start + index)
            .find(|&index| {
                self.delimiter.starts_with(&input[index..])
                    || dash_boundary.starts_with(&input[index + 1..])
            })
            .unwrap_or(input.len())
    }

    /// Check each line of a header section, which ends with CRLF and has no empty line.
    fn check_section(&self, section: &[u8]) -> Result<(), MultipartErrorKind> {
        let (mut count, mut rest) = (0, section);
        // Lines are split at CRLF only, so a bare CR or LF stays in its line, where neither a name nor a value allows it.
        while let Some(end) = memchr::memmem::find(rest, b"\r\n") {
            let line = &rest[..end];
            rest = &rest[end + 2..];
            count += 1;
            if count > self.limits.header_count_limit {
                return Err(MultipartErrorKind::HeaderCountLimit);
            }
            if line
                .first()
                .is_some_and(|&byte| byte == b' ' || byte == b'\t')
            {
                return Err(MultipartErrorKind::FoldedField);
            }
            let colon = memchr::memchr(b':', line).ok_or(MultipartErrorKind::InvalidField)?;
            let (name, value) = (&line[..colon], &line[colon + 1..]);
            if name.is_empty()
                || !name.iter().all(|&byte| is_field_name_byte(byte))
                || !value.iter().all(|&byte| is_field_value_byte(byte))
            {
                return Err(MultipartErrorKind::InvalidField);
            }
        }
        Ok(())
    }
}
