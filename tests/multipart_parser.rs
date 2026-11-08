// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_multipart::{
    Boundary, MultipartError, MultipartErrorKind, MultipartEvent, MultipartLimits, MultipartParser,
    parts,
};

/// One part as the events describe it: its fields, and its content reassembled from every piece.
type Collected = Vec<(Vec<(String, Vec<u8>)>, Vec<u8>)>;

/// Feed `chunks` through a parser as a caller would: append each to a buffer, parse until more input is needed, and drop what was consumed.
fn collect(
    boundary: &str,
    limits: MultipartLimits,
    chunks: &[&[u8]],
) -> Result<Collected, MultipartError> {
    let mut parser = MultipartParser::with_limits(&Boundary::new(boundary).unwrap(), limits);
    let (mut buffer, mut parts, mut open) = (Vec::new(), Collected::new(), false);
    for chunk in chunks {
        buffer.extend_from_slice(chunk);
        loop {
            let parsed = parser.parse(&buffer)?;
            let event = parsed.event;
            let done = match event {
                MultipartEvent::Part(head) => {
                    assert!(!open, "a part begins only after the previous one ends");
                    open = true;
                    parts.push((
                        head.fields()
                            .map(|(name, value)| (name.to_owned(), value.to_vec()))
                            .collect(),
                        Vec::new(),
                    ));
                    false
                }
                MultipartEvent::Content(content) => {
                    assert!(open && !content.is_empty());
                    parts.last_mut().unwrap().1.extend_from_slice(content);
                    false
                }
                MultipartEvent::PartEnd => {
                    assert!(open);
                    open = false;
                    false
                }
                MultipartEvent::End => true,
                MultipartEvent::Incomplete => true,
            };
            buffer.drain(..parsed.consumed);
            if done {
                break;
            }
        }
    }
    parser.finish()?;
    assert!(!open);
    Ok(parts)
}

fn whole(boundary: &str, body: &[u8]) -> Result<Collected, MultipartError> {
    collect(boundary, MultipartLimits::default(), &[body])
}

/// Every way of splitting the body in two, and one byte at a time, gives the same parts as the whole body at once.
fn assert_split_invariant(boundary: &str, body: &[u8]) -> Collected {
    let expected = whole(boundary, body).unwrap();
    for split in 0..=body.len() {
        let (first, second) = body.split_at(split);
        assert_eq!(
            collect(boundary, MultipartLimits::default(), &[first, second]).unwrap(),
            expected,
            "split at {split}"
        );
    }
    let bytes: Vec<&[u8]> = body.chunks(1).collect();
    assert_eq!(
        collect(boundary, MultipartLimits::default(), &bytes).unwrap(),
        expected
    );
    expected
}

const FORM: &[u8] = b"preamble line\r\nmore preamble\r\n--XyZ\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHello, world\r\n--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"caf\xc3\xa9.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n\r\n--XY\r\r\n-\r\n--Xy\x00\xff--XyZ\r\n--XyZ\t \r\n\r\n\r\n--XyZ--\r\nepilogue --XyZ junk";

#[test]
fn parses_parts_however_the_body_is_split() {
    let parts = assert_split_invariant("XyZ", FORM);
    assert_eq!(parts.len(), 3);
    assert_eq!(
        parts[0].0,
        [(
            "Content-Disposition".to_owned(),
            b"form-data; name=\"title\"".to_vec()
        )]
    );
    assert_eq!(parts[0].1, b"Hello, world");
    assert_eq!(
        parts[1].0[0].1,
        b"form-data; name=\"file\"; filename=\"caf\xc3\xa9.bin\"".to_vec(),
        "UTF-8 in a value is kept"
    );
    assert_eq!(
        parts[1].1, b"\r\n--XY\r\r\n-\r\n--Xy\x00\xff--XyZ",
        "partial delimiters, and the boundary not after a line break, are content"
    );
    assert_eq!(
        parts[2],
        (Vec::new(), Vec::new()),
        "a part with no fields and no content, after transport padding"
    );
}

#[test]
fn parses_bodies_that_begin_with_the_boundary_or_end_at_once() {
    let body = b"--b\r\nA: 1\r\n\r\nx\r\n--b--";
    assert_eq!(
        assert_split_invariant("b", body),
        [(vec![("A".to_owned(), b"1".to_vec())], b"x".to_vec())]
    );
    assert_eq!(
        assert_split_invariant("b", b"--b--\r\n"),
        Collected::new(),
        "a body with no parts"
    );
    assert_eq!(
        assert_split_invariant("b", b"\r\n--b--"),
        Collected::new(),
        "a preamble that is only a line break"
    );
    // A preamble line that begins like the boundary but is not it.
    assert_eq!(
        assert_split_invariant("boundary", b"--bound\r\n--boundary--").len(),
        0
    );
}

#[test]
fn reports_events_and_ignores_the_epilogue() {
    let mut parser = MultipartParser::new(&Boundary::new("b").unwrap());
    assert!(!parser.is_finished());
    assert_eq!(parser.limits(), MultipartLimits::default());
    assert_eq!(
        parser.minimum_buffer_size(),
        MultipartLimits::HEADER_SIZE_LIMIT + 3
    );
    let mut input: &[u8] = b"--b\r\n\r\ndata\r\n--b--tail";
    let mut events = Vec::new();
    loop {
        let parsed = parser.parse(input).unwrap();
        input = &input[parsed.consumed..];
        events.push(format!("{:?}", parsed.event));
        if parsed.event == MultipartEvent::End {
            break;
        }
    }
    assert_eq!(
        events,
        [
            "Part(PartHead { fields: [] })",
            "Content(<4 bytes>)",
            "PartEnd",
            "End"
        ]
    );
    assert_eq!(format!("{:?}", MultipartEvent::Incomplete), "Incomplete");
    assert_eq!(input, b"tail");
    let parsed = parser.parse(input).unwrap();
    assert_eq!(
        (parsed.consumed, parsed.event),
        (4, MultipartEvent::End),
        "the epilogue is consumed and ignored"
    );
    assert!(parser.is_finished() && parser.finish().is_ok());
    assert_eq!(
        parser.clone().parse(b"").unwrap().event,
        MultipartEvent::End
    );
    assert!(format!("{parser:?}").contains("MultipartParser"));
}

#[test]
fn waits_for_input_that_could_still_form_a_boundary() {
    let mut parser = MultipartParser::new(&Boundary::new("boundary").unwrap());
    for prefix in [&b""[..], b"-", b"--bound"] {
        let parsed = parser.parse(prefix).unwrap();
        assert_eq!(
            (parsed.consumed, parsed.event),
            (0, MultipartEvent::Incomplete),
            "{prefix:?}"
        );
    }
    let parsed = parser.parse(b"--boundary").unwrap();
    assert_eq!(
        (parsed.consumed, parsed.event),
        (10, MultipartEvent::Incomplete)
    );
    for suffix in [&b"-"[..], b"\r", b"  ", b""] {
        assert_eq!(
            parser.clone().parse(suffix).unwrap().event,
            MultipartEvent::Incomplete,
            "{suffix:?}"
        );
    }
    let parsed = parser.parse(b"\r\nName: value\r\n").unwrap();
    assert_eq!(
        (parsed.consumed, parsed.event),
        (2, MultipartEvent::Incomplete),
        "a header section is returned whole"
    );
    let parsed = parser.parse(b"Name: value\r\n").unwrap();
    assert_eq!(
        (parsed.consumed, parsed.event),
        (0, MultipartEvent::Incomplete)
    );
    let parsed = parser.parse(b"Name: value\r\n\r\nab\r\n--bou").unwrap();
    assert!(
        matches!(parsed.event, MultipartEvent::Part(head) if head.len() == 1 && !head.is_empty())
    );
    let parsed = parser.parse(b"ab\r\n--bou").unwrap();
    assert_eq!(
        (parsed.consumed, parsed.event),
        (2, MultipartEvent::Content(b"ab")),
        "the possible delimiter is held back"
    );
    let parsed = parser.parse(b"\r\n--bou").unwrap();
    assert_eq!(
        (parsed.consumed, parsed.event),
        (0, MultipartEvent::Incomplete)
    );
}

fn error(boundary: &str, limits: MultipartLimits, body: &[u8]) -> (MultipartErrorKind, u64) {
    let error = collect(boundary, limits, &[body]).unwrap_err();
    (error.kind(), error.offset())
}

#[test]
fn rejects_malformed_bodies_with_their_offsets() {
    let limits = MultipartLimits::default();
    let cases: [(&[u8], MultipartErrorKind, u64); 12] = [
        (
            b"--b\r\n folded: x\r\n\r\n",
            MultipartErrorKind::FoldedField,
            5,
        ),
        (
            b"--b\r\nA: 1\r\n\tcontinued\r\n\r\n",
            MultipartErrorKind::FoldedField,
            5,
        ),
        (
            b"--b\r\nno colon\r\n\r\n",
            MultipartErrorKind::InvalidField,
            5,
        ),
        (
            b"--b\r\n: empty name\r\n\r\n",
            MultipartErrorKind::InvalidField,
            5,
        ),
        (
            b"--b\r\nBad Name: x\r\n\r\n",
            MultipartErrorKind::InvalidField,
            5,
        ),
        (
            b"--b\r\nA: bare\nB: lf\r\n\r\n",
            MultipartErrorKind::InvalidField,
            5,
        ),
        (
            b"--b\r\nA: stray\r\r\n\r\n",
            MultipartErrorKind::InvalidField,
            5,
        ),
        (
            b"--b\r\nA: \x01\r\n\r\n",
            MultipartErrorKind::InvalidField,
            5,
        ),
        (b"--bX\r\n", MultipartErrorKind::InvalidDelimiter, 3),
        (
            b"--b \r\n\r\nx\r\n--b-x",
            MultipartErrorKind::InvalidDelimiter,
            14,
        ),
        (
            b"--b\r\n\r\ncontent\r\n--b and more\r\n",
            MultipartErrorKind::InvalidDelimiter,
            20,
        ),
        (
            b"--b\r\n\r\ntruncated",
            MultipartErrorKind::UnexpectedEnd,
            16,
        ),
    ];
    for (body, kind, offset) in cases {
        assert_eq!(
            error("b", limits, body),
            (kind, offset),
            "{:?}",
            String::from_utf8_lossy(body)
        );
    }
    for truncated in [
        &b""[..],
        b"no boundary",
        b"--b",
        b"--b\r\nA: 1\r\n",
        b"--b\r\n\r\nx\r\n--b",
    ] {
        assert_eq!(
            error("b", limits, truncated).0,
            MultipartErrorKind::UnexpectedEnd,
            "{truncated:?}"
        );
    }
    let error = collect("b", limits, &[b"--b\r\n\r\n"]).unwrap_err();
    assert_eq!(
        error.to_string(),
        "multipart body ends before its closing boundary at byte 7"
    );
    let error: Box<dyn std::error::Error> = Box::new(error);
    assert!(error.source().is_none());
}

#[test]
fn applies_its_limits() {
    let limits = MultipartLimits {
        preamble_size_limit: 4,
        header_size_limit: 13,
        header_count_limit: 2,
        part_count_limit: 2,
    };
    assert!(
        collect("b", limits, &[b"1234\r\n--b--"]).is_ok(),
        "a preamble at its limit"
    );
    assert_eq!(
        error("b", limits, b"12345\r\n--b--"),
        (MultipartErrorKind::PreambleSizeLimit, 0)
    );
    assert_eq!(
        error("b", limits, b"12345678"),
        (MultipartErrorKind::PreambleSizeLimit, 0),
        "found before the boundary arrives"
    );
    assert!(
        collect("b", limits, &[b"--b\r\nA: 1\r\nB: 22\r\n\r\n\r\n--b--"]).is_ok(),
        "a header section at its limit"
    );
    assert_eq!(
        error("b", limits, b"--b\r\nA: 1\r\nB: 223\r\n\r\n\r\n--b--"),
        (MultipartErrorKind::HeaderSizeLimit, 5)
    );
    assert_eq!(
        error("b", limits, b"--b\r\nA: 1234567890123"),
        (MultipartErrorKind::HeaderSizeLimit, 5),
        "found before the section ends"
    );
    let counted = MultipartLimits {
        header_count_limit: 2,
        ..MultipartLimits::default()
    };
    assert_eq!(
        error("b", counted, b"--b\r\nA: 1\r\nB: 2\r\nC: 3\r\n\r\n"),
        (MultipartErrorKind::HeaderCountLimit, 5)
    );
    assert_eq!(
        error("b", limits, b"--b              \r\n"),
        (MultipartErrorKind::HeaderSizeLimit, 3),
        "padding counts toward the header size"
    );
    assert!(collect("b", limits, &[b"--b\r\n\r\n\r\n--b\r\n\r\n\r\n--b--"]).is_ok());
    assert_eq!(
        error("b", limits, b"--b\r\n\r\n\r\n--b\r\n\r\n\r\n--b\r\n"),
        (MultipartErrorKind::PartCountLimit, 21)
    );
}

#[test]
fn parts_of_a_complete_body() {
    let boundary = Boundary::new("XyZ").unwrap();
    let collected: Vec<_> = parts(FORM, &boundary).map(Result::unwrap).collect();
    assert_eq!(collected.len(), 3);
    assert_eq!(collected[0].content(), b"Hello, world");
    assert_eq!(
        collected[1].head().fields().nth(1),
        Some(("Content-Type", &b"application/octet-stream"[..]))
    );
    assert_eq!(
        format!("{:?}", collected[0]),
        "Part { head: PartHead { fields: [(\"Content-Disposition\", \"<23 bytes>\")] }, content: <12 bytes> }"
    );
    assert_eq!(
        collected[0].head().as_bytes(),
        b"Content-Disposition: form-data; name=\"title\"\r\n"
    );
    assert_eq!(collected[0].head().into_iter().count(), 1);
    let mut truncated = parts(b"--XyZ\r\n\r\nx\r\n--XyZ\r\n\r\nunfinished", &boundary);
    assert_eq!(truncated.next().unwrap().unwrap().content(), b"x");
    assert_eq!(
        truncated.next().unwrap().unwrap_err().kind(),
        MultipartErrorKind::UnexpectedEnd
    );
    assert!(truncated.next().is_none(), "an error ends the parts");
    let mut malformed = parts(b"--XyZ\r\nbad\r\n\r\n", &boundary);
    assert_eq!(
        malformed.next().unwrap().unwrap_err().kind(),
        MultipartErrorKind::InvalidField
    );
    let limits = MultipartLimits {
        part_count_limit: 1,
        ..MultipartLimits::default()
    };
    let mut limited = base_web_multipart::Parts::with_limits(FORM, &boundary, limits);
    assert!(limited.next().unwrap().is_ok());
    assert_eq!(
        limited.next().unwrap().unwrap_err().kind(),
        MultipartErrorKind::PartCountLimit
    );
    let mut empty = parts(b"--XyZ--", &boundary).clone();
    assert!(empty.next().is_none());
    assert!(format!("{empty:?}").starts_with("Parts"));
}

#[test]
fn every_error_describes_itself() {
    let default = MultipartLimits::default();
    let cases: [(MultipartErrorKind, MultipartLimits, &[u8], &str); 7] = [
        (
            MultipartErrorKind::PreambleSizeLimit,
            MultipartLimits {
                preamble_size_limit: 0,
                ..default
            },
            b"x\r\n--b",
            "multipart preamble exceeds its size limit",
        ),
        (
            MultipartErrorKind::HeaderSizeLimit,
            MultipartLimits {
                header_size_limit: 1,
                ..default
            },
            b"--b\r\nA: 1\r\n\r\n",
            "multipart part header section exceeds its size limit",
        ),
        (
            MultipartErrorKind::HeaderCountLimit,
            MultipartLimits {
                header_count_limit: 0,
                ..default
            },
            b"--b\r\nA: 1\r\n\r\n",
            "multipart part header section exceeds its field count limit",
        ),
        (
            MultipartErrorKind::PartCountLimit,
            MultipartLimits {
                part_count_limit: 0,
                ..default
            },
            b"--b\r\n",
            "multipart body exceeds its part count limit",
        ),
        (
            MultipartErrorKind::InvalidField,
            default,
            b"--b\r\nx\r\n\r\n",
            "invalid multipart part header field",
        ),
        (
            MultipartErrorKind::FoldedField,
            default,
            b"--b\r\n x\r\n\r\n",
            "folded multipart part header field",
        ),
        (
            MultipartErrorKind::InvalidDelimiter,
            default,
            b"--b?",
            "invalid multipart boundary line",
        ),
    ];
    for (kind, limits, body, description) in cases {
        let error = collect("b", limits, &[body]).unwrap_err();
        assert_eq!(error.kind(), kind);
        assert!(error.to_string().starts_with(description), "{error}");
    }
}
