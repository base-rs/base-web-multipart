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
    finder: Finder<'static>,
    limits: MultipartLimits,
    state: State,
    /// The bytes consumed before the current call, for the offsets of errors.
    offset: u64,
    preamble_size: usize,
    part_count: usize,
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
    /// In a part's header section, with the bytes of the input already searched for its end.
    Headers {
        searched: usize,
    },
    Content,
    End,
}

impl MultipartParser {
    /// A parser for a body with this boundary, with the default limits.
    pub fn new(boundary: &Boundary) -> Self {
        Self::with_limits(boundary, MultipartLimits::default())
    }

    /// A parser for a body with this boundary and these limits.
    pub fn with_limits(boundary: &Boundary, limits: MultipartLimits) -> Self {
        let delimiter = [b"\r\n--", boundary.as_str().as_bytes()].concat();
        let finder = Finder::new(&delimiter).into_owned();
        Self {
            delimiter,
            finder,
            limits,
            state: State::Start,
            offset: 0,
            preamble_size: 0,
            part_count: 0,
        }
    }

    /// The limits this parser applies.
    pub fn limits(&self) -> MultipartLimits {
        self.limits
    }

    /// The input a caller must be able to buffer: given at least this many bytes, [`Self::parse`] always consumes some or reports an event other than [`MultipartEvent::Incomplete`].
    pub fn minimum_buffer_size(&self) -> usize {
        (self.limits.header_size_limit + 3).max(self.delimiter.len())
    }

    /// Whether the closing boundary has been reached.
    pub fn is_finished(&self) -> bool {
        self.state == State::End
    }

    /// Parse the next event from `input`, which must begin with the bytes the previous call did not consume.
    ///
    /// # Errors
    ///
    /// A [`MultipartError`] when the body is malformed or exceeds a limit. The parser should not be used after an error.
    pub fn parse<'a>(&mut self, input: &'a [u8]) -> Result<Parsed<'a>, MultipartError> {
        let mut consumed = 0;
        let event = self
            .step(input, &mut consumed)
            .map_err(|kind| MultipartError::new(kind, self.offset + consumed as u64))?;
        self.offset += consumed as u64;
        Ok(Parsed { consumed, event })
    }

    /// Check that the body was complete, once its input has ended.
    ///
    /// # Errors
    ///
    /// [`MultipartErrorKind::UnexpectedEnd`] when the closing boundary was not reached.
    pub fn finish(&self) -> Result<(), MultipartError> {
        if self.is_finished() {
            Ok(())
        } else {
            Err(MultipartError::new(
                MultipartErrorKind::UnexpectedEnd,
                self.offset,
            ))
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
                    let (skipped, found) = match self.finder.find(rest) {
                        Some(index) => (index, true),
                        None => (safe_length(rest, &self.delimiter), false),
                    };
                    self.preamble_size += skipped;
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
                        self.state = State::End;
                        return Ok(MultipartEvent::End);
                    }
                    let spaces = rest
                        .iter()
                        .take_while(|&&byte| byte == b' ' || byte == b'\t')
                        .count();
                    let padding = padding + spaces;
                    if padding > self.limits.header_size_limit {
                        return Err(MultipartErrorKind::HeaderSizeLimit);
                    }
                    *consumed += spaces;
                    self.state = State::Delimiter { padding };
                    let rest = &rest[spaces..];
                    if rest.starts_with(b"\r\n") {
                        self.part_count += 1;
                        if self.part_count > self.limits.part_count_limit {
                            return Err(MultipartErrorKind::PartCountLimit);
                        }
                        *consumed += 2;
                        self.state = State::Headers { searched: 0 };
                    } else if rest.is_empty() || rest == b"\r" || (padding == 0 && rest == b"-") {
                        return Ok(MultipartEvent::Incomplete);
                    } else {
                        return Err(MultipartErrorKind::InvalidDelimiter);
                    }
                }
                State::Headers { searched } => {
                    let section = if rest.starts_with(b"\r\n") {
                        Some(0)
                    } else {
                        let start = searched.saturating_sub(3);
                        memchr::memmem::find(&rest[start..], b"\r\n\r\n")
                            .map(|index| start + index + 2)
                    };
                    let Some(length) = section else {
                        if rest.len() > self.limits.header_size_limit + 2 {
                            return Err(MultipartErrorKind::HeaderSizeLimit);
                        }
                        self.state = State::Headers {
                            searched: rest.len(),
                        };
                        return Ok(MultipartEvent::Incomplete);
                    };
                    if length > self.limits.header_size_limit {
                        return Err(MultipartErrorKind::HeaderSizeLimit);
                    }
                    let section = &rest[..length];
                    self.check_section(section)?;
                    *consumed += length + 2;
                    self.state = State::Content;
                    return Ok(MultipartEvent::Part(PartHead::new(section)));
                }
                State::Content => {
                    return Ok(match self.finder.find(rest) {
                        Some(0) => {
                            *consumed += self.delimiter.len();
                            self.state = State::Delimiter { padding: 0 };
                            MultipartEvent::PartEnd
                        }
                        Some(index) => {
                            *consumed += index;
                            MultipartEvent::Content(&rest[..index])
                        }
                        None => match safe_length(rest, &self.delimiter) {
                            0 => MultipartEvent::Incomplete,
                            length => {
                                *consumed += length;
                                MultipartEvent::Content(&rest[..length])
                            }
                        },
                    });
                }
                State::End => {
                    // The epilogue is ignored.
                    *consumed += rest.len();
                    return Ok(MultipartEvent::End);
                }
            }
        }
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

/// The bytes at the start of `input` that cannot be the beginning of `delimiter`, which begins with CR and contains no other: everything before the last CR from which the rest of the input could still become the delimiter.
fn safe_length(input: &[u8], delimiter: &[u8]) -> usize {
    let start = input.len().saturating_sub(delimiter.len() - 1);
    memchr::memchr_iter(b'\r', &input[start..])
        .map(|index| start + index)
        .find(|&index| delimiter.starts_with(&input[index..]))
        .unwrap_or(input.len())
}
