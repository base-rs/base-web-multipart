// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Every way of splitting a body gives the same events, with the same offsets, and the same error at the same offset, as the whole body at once.

use std::collections::HashSet;

use base_web_multipart::{
    Boundary, MultipartErrorKind, MultipartEvent, MultipartLimits, MultipartParser,
};

/// One event as the caller sees it, with the offset in the body where it ends. Adjacent pieces of content are joined, since content may arrive in any number of pieces.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Seen {
    Part(Vec<u8>, u64),
    Content(Vec<u8>, u64),
    PartEnd(u64),
    End(u64),
}

/// The events a body produced, and how it ended: `Ok` when `finish` accepted it, or the error's kind and offset.
type Trace = (Vec<Seen>, Result<(), (MultipartErrorKind, u64)>);

/// Feed `chunks` through a parser as a caller would: append each to a buffer, parse until more input is needed, drop what was consumed, and call `finish` once the input ends.
fn trace(boundary: &Boundary, limits: MultipartLimits, chunks: &[&[u8]]) -> Trace {
    let mut parser = MultipartParser::with_limits(boundary, limits);
    let (mut buffer, mut seen, mut offset) = (Vec::new(), Vec::new(), 0u64);
    for chunk in chunks {
        buffer.extend_from_slice(chunk);
        loop {
            let parsed = match parser.parse(&buffer) {
                Ok(parsed) => parsed,
                Err(error) => {
                    assert_eq!(parser.finish(), Err(error), "the error is reported again");
                    return (seen, Err((error.kind(), error.offset())));
                }
            };
            offset += parsed.consumed as u64;
            let ended = matches!(seen.last(), Some(Seen::End(_)));
            match parsed.event {
                MultipartEvent::Part(head) => {
                    seen.push(Seen::Part(head.as_bytes().to_vec(), offset))
                }
                MultipartEvent::Content(piece) => {
                    assert!(!piece.is_empty());
                    match seen.last_mut() {
                        Some(Seen::Content(content, end)) => {
                            content.extend_from_slice(piece);
                            *end = offset;
                        }
                        _ => seen.push(Seen::Content(piece.to_vec(), offset)),
                    }
                }
                MultipartEvent::PartEnd => seen.push(Seen::PartEnd(offset)),
                // The epilogue reports the end again for every call.
                MultipartEvent::End if ended => {}
                MultipartEvent::End => seen.push(Seen::End(offset)),
                MultipartEvent::Incomplete => {}
            }
            let (consumed, waiting) = (
                parsed.consumed,
                matches!(
                    parsed.event,
                    MultipartEvent::Incomplete | MultipartEvent::End
                ),
            );
            buffer.drain(..consumed);
            if waiting {
                break;
            }
        }
    }
    (
        seen,
        parser
            .finish()
            .map_err(|error| (error.kind(), error.offset())),
    )
}

/// Check that the body split in two at every point, and fed a byte at a time, gives the same trace as the whole body, and return it.
fn assert_split_invariant(boundary: &Boundary, limits: MultipartLimits, body: &[u8]) -> Trace {
    let expected = trace(boundary, limits, &[body]);
    let context = || format!("{:?} with {limits:?}", String::from_utf8_lossy(body));
    for split in 0..=body.len() {
        let (first, second) = body.split_at(split);
        assert_eq!(
            trace(boundary, limits, &[first, second]),
            expected,
            "{} split at {split}",
            context()
        );
    }
    let bytes: Vec<&[u8]> = body.chunks(1).collect();
    assert_eq!(
        trace(boundary, limits, &bytes),
        expected,
        "{} a byte at a time",
        context()
    );
    expected
}

/// Check the split invariant, and that the body split in three at every pair of points gives the same trace too.
fn assert_thoroughly_split_invariant(
    boundary: &Boundary,
    limits: MultipartLimits,
    body: &[u8],
) -> Trace {
    let expected = assert_split_invariant(boundary, limits, body);
    for first in 0..=body.len() {
        for second in first..=body.len() {
            let chunks = [&body[..first], &body[first..second], &body[second..]];
            assert_eq!(
                trace(boundary, limits, &chunks),
                expected,
                "{:?} split at {first} and {second}",
                String::from_utf8_lossy(body)
            );
        }
    }
    expected
}

fn boundary(text: &str) -> Boundary {
    Boundary::new(text).unwrap()
}

const SMALL: MultipartLimits = MultipartLimits {
    preamble_size_limit: 4,
    header_size_limit: 8,
    header_count_limit: 1,
    part_count_limit: 2,
};

#[test]
fn reports_the_preamble_limit_at_the_start_of_the_body() {
    // The preamble begins the body, so its limit is reported at its start, as it is for the whole body, rather than where the input that exceeds it began.
    for body in [
        &b"12345\r\n--b\r\n\r\n\r\n--b--"[..],
        b"1234567890",
        b"1234\r\r\n--b--",
        b"\r\n\r\n\r\n\r\n--b--",
    ] {
        assert_eq!(
            assert_thoroughly_split_invariant(&boundary("b"), SMALL, body),
            (Vec::new(), Err((MultipartErrorKind::PreambleSizeLimit, 0))),
            "{body:?}"
        );
    }
    assert_eq!(
        assert_thoroughly_split_invariant(&boundary("b"), SMALL, b"1234\r\n--b--").1,
        Ok(()),
        "a preamble at its limit"
    );
    let unlimited = MultipartLimits {
        preamble_size_limit: usize::MAX,
        ..SMALL
    };
    assert_eq!(
        assert_thoroughly_split_invariant(&boundary("b"), unlimited, b"preamble\r\n--b--").1,
        Ok(())
    );
}

#[test]
fn counts_the_preamble_before_a_bare_line_break() {
    // The preamble before the break is counted first, as it is when the body is split before the break, so a long one exceeds its limit.
    let cases: [(&[u8], MultipartErrorKind, u64); 5] = [
        (
            b"12\n--b\r\n\r\n\r\n--b--",
            MultipartErrorKind::InvalidDelimiter,
            2,
        ),
        (b"1234\n--b--", MultipartErrorKind::InvalidDelimiter, 4),
        (b"12345\n--b--", MultipartErrorKind::PreambleSizeLimit, 0),
        (b"12\r\n\n--b--", MultipartErrorKind::InvalidDelimiter, 4),
        (b"123\r\n\r--b--", MultipartErrorKind::PreambleSizeLimit, 0),
    ];
    for (body, kind, offset) in cases {
        assert_eq!(
            assert_thoroughly_split_invariant(&boundary("b"), SMALL, body),
            (Vec::new(), Err((kind, offset))),
            "{body:?}"
        );
    }
}

#[test]
fn reports_the_content_before_a_bare_line_break() {
    // The content before the break is reported first, as it is when the body is split before the break, and then the error at the break.
    let body = b"--b\r\nA: 1\r\n\r\nabc\n--b\r\n\r\nevil\r\n--b--";
    assert_eq!(
        assert_thoroughly_split_invariant(&boundary("b"), MultipartLimits::default(), body),
        (
            vec![
                Seen::Part(b"A: 1\r\n".to_vec(), 11),
                Seen::Content(b"abc".to_vec(), 16),
            ],
            Err((MultipartErrorKind::InvalidDelimiter, 16)),
        )
    );
    let body = b"--b\r\n\r\nx\r\r\r--b--";
    assert_eq!(
        assert_thoroughly_split_invariant(&boundary("b"), MultipartLimits::default(), body),
        (
            vec![
                Seen::Part(Vec::new(), 5),
                Seen::Content(b"x\r\r".to_vec(), 10),
            ],
            Err((MultipartErrorKind::InvalidDelimiter, 10)),
        ),
        "the bare CRs before the one that precedes the boundary are content"
    );
    let body = b"--b\r\n\r\n\n--b--";
    assert_eq!(
        assert_thoroughly_split_invariant(&boundary("b"), MultipartLimits::default(), body),
        (
            vec![Seen::Part(Vec::new(), 5)],
            Err((MultipartErrorKind::InvalidDelimiter, 7)),
        ),
        "no content before the break"
    );
}

#[test]
fn reports_the_padding_limit_at_the_start_of_the_padding() {
    // The padding counts toward the header section, and is reported at its start, as it is for the whole body, rather than where the input that exceeds it began.
    for (body, offset) in [
        (&b"--b         \r\n"[..], 3),
        (b"--b\r\n\r\n\r\n--b--         ", 14),
    ] {
        assert_eq!(
            assert_thoroughly_split_invariant(&boundary("b"), SMALL, body).1,
            Err((MultipartErrorKind::HeaderSizeLimit, offset)),
            "{body:?}"
        );
    }
}

/// The pieces bodies are built from: the boundary and the line breaks, dashes, padding, and header lines around it, so that near misses are common.
fn pieces(boundary: &str) -> Vec<Vec<u8>> {
    let dash_boundary = format!("--{boundary}");
    [
        &dash_boundary[..],
        "\r\n",
        "\r",
        "\n",
        "-",
        " ",
        "x",
        "A: 1",
    ]
    .iter()
    .map(|piece| piece.as_bytes().to_vec())
    .collect()
}

fn limits() -> [MultipartLimits; 5] {
    [
        MultipartLimits::default(),
        SMALL,
        MultipartLimits {
            header_count_limit: 0,
            ..MultipartLimits::default()
        },
        MultipartLimits {
            preamble_size_limit: 0,
            header_size_limit: 0,
            header_count_limit: 0,
            part_count_limit: 0,
        },
        MultipartLimits {
            preamble_size_limit: usize::MAX,
            header_size_limit: usize::MAX,
            header_count_limit: usize::MAX,
            part_count_limit: usize::MAX,
        },
    ]
}

/// Check that the generated bodies reached every outcome, so that the invariant was checked for each.
fn assert_every_outcome(seen: &HashSet<Option<MultipartErrorKind>>) {
    for kind in [
        None,
        Some(MultipartErrorKind::PreambleSizeLimit),
        Some(MultipartErrorKind::HeaderSizeLimit),
        Some(MultipartErrorKind::HeaderCountLimit),
        Some(MultipartErrorKind::PartCountLimit),
        Some(MultipartErrorKind::InvalidField),
        Some(MultipartErrorKind::FoldedField),
        Some(MultipartErrorKind::InvalidDelimiter),
        Some(MultipartErrorKind::UnexpectedEnd),
    ] {
        assert!(seen.contains(&kind), "{kind:?}");
    }
}

#[test]
fn every_short_body_parses_the_same_however_it_is_split() {
    let mut seen = HashSet::new();
    for text in ["b", "-", "a b"] {
        let boundary = boundary(text);
        let pieces = pieces(text);
        // Every sequence of up to four pieces, after nothing, the beginning of a part, or the beginning of its content.
        let prefixes = [
            String::new(),
            format!("--{text}\r\n"),
            format!("--{text}\r\n\r\n"),
        ];
        for prefix in prefixes {
            let mut bodies = vec![prefix.into_bytes()];
            for _ in 0..4 {
                let longer: Vec<Vec<u8>> = bodies
                    .iter()
                    .flat_map(|body| {
                        pieces
                            .iter()
                            .map(move |piece| [body.as_slice(), piece].concat())
                    })
                    .collect();
                for body in &longer {
                    for limits in limits() {
                        let (_, result) = assert_split_invariant(&boundary, limits, body);
                        seen.insert(result.err().map(|(kind, _)| kind));
                    }
                }
                bodies = longer;
            }
        }
    }
    assert_every_outcome(&seen);
}

/// A small deterministic generator, so that failures reproduce.
struct Random(u64);

impl Random {
    fn below(&mut self, limit: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % limit as u64) as usize
    }
}

#[test]
fn longer_generated_bodies_parse_the_same_however_they_are_split() {
    let mut random = Random(0x5eed_0fb0_d1e5);
    let (mut parsed, mut outcomes) = (0, HashSet::new());
    for text in ["b", "-", "a-b", "a b"] {
        let boundary = boundary(text);
        let mut pieces = pieces(text);
        pieces.extend([
            b"\r\n\r\n".to_vec(),
            b"\t".to_vec(),
            b"--".to_vec(),
            b" x: y".to_vec(),
        ]);
        for _ in 0..2000 {
            let mut body = Vec::new();
            if random.below(2) == 0 {
                body.extend_from_slice(format!("--{text}\r\n").as_bytes());
            }
            for _ in 0..random.below(16) {
                body.extend_from_slice(&pieces[random.below(pieces.len())]);
            }
            for limits in limits() {
                let (seen, result) = assert_split_invariant(&boundary, limits, &body);
                parsed += usize::from(!seen.is_empty());
                outcomes.insert(result.err().map(|(kind, _)| kind));
            }
        }
    }
    assert!(parsed > 1000, "many bodies have parts: {parsed}");
    assert_every_outcome(&outcomes);
}
