// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_message::{FieldErrorKind, FieldSection, FieldType, FieldValueError};
use base_web_media_type::MediaType;
use base_web_multipart::{
    Boundary, ContentDisposition, ContentTransferEncoding, ContentType, PartHeaders, parts,
};
use std::borrow::Cow;

fn disposition(value: &[u8]) -> Result<ContentDisposition<'_>, FieldValueError> {
    ContentDisposition::parse(value)
}

#[test]
fn parses_dispositions_and_their_parameters() {
    let parsed = disposition(
        b"Form-Data ;Name = \"a \\\"quoted\\\\ name\\\"\" ; FILENAME=plain.txt ;  size=12  ",
    )
    .unwrap();
    assert_eq!(
        parsed.disposition_type(),
        "form-data",
        "the type is lowercased"
    );
    assert_eq!(
        parsed.name(),
        Some("a \"quoted\\ name\""),
        "quoted pairs are unescaped"
    );
    assert_eq!(parsed.filename(), Some("plain.txt"));
    assert_eq!(
        parsed.parameter("SIZE"),
        Some("12"),
        "names are compared case-insensitively"
    );
    assert_eq!(
        parsed.parameters().collect::<Vec<_>>(),
        [
            ("name", "a \"quoted\\ name\""),
            ("filename", "plain.txt"),
            ("size", "12")
        ]
    );
    let parsed = disposition(b"inline").unwrap();
    assert_eq!(
        (
            parsed.disposition_type(),
            parsed.parameters().len(),
            parsed.name()
        ),
        ("inline", 0, None)
    );
    // UTF-8 in a quoted filename, as browsers send it, is kept, and the parse borrows when nothing is unescaped.
    let value = "attachment; filename=\"r\u{e9}sum\u{e9}.pdf\"".as_bytes();
    let parsed = disposition(value).unwrap();
    assert_eq!(parsed.filename(), Some("r\u{e9}sum\u{e9}.pdf"));
    let owned: ContentDisposition<'static> = parsed.clone().into_owned();
    assert_eq!(owned, parsed);
    assert_eq!(
        format!("{owned:?}"),
        "ContentDisposition { disposition_type: \"attachment\", parameters: [(\"filename\", \"<12 bytes>\")] }"
    );
}

#[test]
fn rejects_malformed_dispositions() {
    let cases: [(&[u8], &str); 12] = [
        (b"", "missing value"),
        (b"; name=a", "missing value"),
        (b"form-data name=a", "expected a parameter separator"),
        (b"form-data; =a", "missing parameter name"),
        (b"form-data; name", "expected a parameter value"),
        (b"form-data; name=", "missing parameter value"),
        (b"form-data; name=\"open", "unterminated quoted string"),
        (b"form-data; name=\"open\\", "unterminated quoted string"),
        (b"form-data; name=a; NAME=b", "duplicate parameter"),
        (
            b"form-data; name=\"a\x01\"",
            "value contains a control character",
        ),
        (b"form-data; name=\"\xff\"", "value is not UTF-8"),
        (b"form-data; name=a b", "expected a parameter separator"),
    ];
    for (value, reason) in cases {
        assert_eq!(
            disposition(value).unwrap_err().reason(),
            reason,
            "{:?}",
            String::from_utf8_lossy(value)
        );
    }
    let many: String = std::iter::once("form-data".to_owned())
        .chain((0..=64).map(|index| format!("; p{index}=v")))
        .collect();
    assert_eq!(
        disposition(many.as_bytes()).unwrap_err().reason(),
        "too many parameters"
    );
}

#[test]
fn builds_and_writes_dispositions() {
    let written = ContentDisposition::new("Attachment")
        .unwrap()
        .with_parameter("FileName", "a\\b \"c\" caf\u{e9}")
        .unwrap();
    assert_eq!(
        written.to_string(),
        "attachment; filename=\"a\\\\b \\\"c\\\" caf\u{e9}\""
    );
    assert_eq!(
        disposition(written.to_string().as_bytes()).unwrap(),
        written,
        "what is written parses back"
    );
    assert_eq!(
        ContentDisposition::new("not a token").unwrap_err().reason(),
        "disposition type is not a token"
    );
    let form = ContentDisposition::form_data("field").unwrap();
    assert_eq!(form.to_string(), "form-data; name=\"field\"");
    assert_eq!(
        ContentDisposition::form_data("line\r\nbreak")
            .unwrap_err()
            .reason(),
        "parameter value contains a control character"
    );
    assert_eq!(
        form.clone()
            .with_parameter("bad name", "x")
            .unwrap_err()
            .reason(),
        "parameter name is not a token"
    );
    assert_eq!(
        form.clone()
            .with_parameter("NAME", "x")
            .unwrap_err()
            .reason(),
        "duplicate parameter"
    );
    let full = (0..63).fold(form, |disposition, index| {
        disposition
            .with_parameter(&format!("p{index}"), "v")
            .unwrap()
    });
    assert_eq!(
        full.with_parameter("one-more", "v").unwrap_err().reason(),
        "too many parameters"
    );
}

#[test]
fn parses_content_types_and_transfer_encodings() {
    let content_type = ContentType::parse(b"text/plain; charset=utf-8").unwrap();
    assert_eq!(
        content_type.media_type().parameter("charset"),
        Some("utf-8")
    );
    assert_eq!(
        content_type.clone().into_media_type().essence(),
        "text/plain"
    );
    assert_eq!(
        ContentType::from(MediaType::IMAGE_PNG).media_type(),
        &MediaType::IMAGE_PNG
    );
    assert_eq!(
        ContentType::parse(b"not a type").unwrap_err().reason(),
        "invalid media type"
    );
    assert_eq!(
        ContentType::parse(b"text/\xff").unwrap_err().reason(),
        "value is not visible ASCII"
    );
    assert_eq!(
        ContentTransferEncoding::parse(b"\xff")
            .unwrap_err()
            .reason(),
        "value is not visible ASCII"
    );
    for (value, encoding) in [
        (&b"7BIT"[..], ContentTransferEncoding::SevenBit),
        (b"8bit", ContentTransferEncoding::EightBit),
        (b"binary", ContentTransferEncoding::Binary),
    ] {
        let parsed = ContentTransferEncoding::parse(value).unwrap();
        assert_eq!(parsed, encoding);
        assert!(
            parsed
                .as_str()
                .eq_ignore_ascii_case(std::str::from_utf8(value).unwrap())
        );
    }
    for unsupported in [&b"base64"[..], b"quoted-printable", b"x-custom"] {
        assert_eq!(
            ContentTransferEncoding::parse(unsupported)
                .unwrap_err()
                .reason(),
            "unsupported content transfer encoding"
        );
    }
}

/// A section that reads one more field alongside the standard ones.
#[derive(Debug, FieldSection)]
struct UploadHeaders<'a> {
    #[fields(flatten)]
    standard: PartHeaders<'a>,
    #[field("content-id")]
    content_id: Option<Identifier<'a>>,
}

#[derive(Debug, PartialEq)]
struct Identifier<'a>(&'a str);

impl<'a> FieldType<'a> for Identifier<'a> {
    type Combination = base_message::combination::Singleton;
    fn parse(value: &'a [u8]) -> Result<Self, FieldValueError> {
        base_message::ascii_text(value).map(Self)
    }
}

#[test]
fn interprets_part_headers_from_a_body() {
    let body = b"--b\r\nContent-Disposition: form-data; name=\"upload\"; filename=\"a.txt\"\r\nContent-Type: text/plain\r\nContent-ID: <id@example>\r\nContent-Transfer-Encoding: binary\r\n\r\nx\r\n--b--";
    let boundary = Boundary::new("b").unwrap();
    let part = parts(body, &boundary).next().unwrap().unwrap();
    let headers = UploadHeaders::interpret(part.head()).unwrap();
    let disposition = headers.standard.content_disposition.as_ref().unwrap();
    assert_eq!(
        (disposition.name(), disposition.filename()),
        (Some("upload"), Some("a.txt"))
    );
    assert!(matches!(
        disposition.filename().map(Cow::Borrowed),
        Some(Cow::Borrowed("a.txt"))
    ));
    assert_eq!(
        headers
            .standard
            .content_type
            .as_ref()
            .map(|content_type| content_type.media_type().essence()),
        Some("text/plain")
    );
    assert_eq!(
        headers.standard.content_transfer_encoding,
        Some(ContentTransferEncoding::Binary)
    );
    assert_eq!(headers.content_id, Some(Identifier("<id@example>")));
    assert!(
        format!("{headers:?}").contains("<6 bytes>"),
        "the disposition's values are not shown"
    );
    let error = PartHeaders::interpret([
        ("Content-Type", "text/plain"),
        ("content-type", "text/html"),
    ])
    .unwrap_err();
    assert_eq!(error.kind(), FieldErrorKind::Duplicate { first: 0 });
    let error = PartHeaders::interpret([("Content-Transfer-Encoding", "base64")]).unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid content-transfer-encoding field at position 0: unsupported content transfer encoding"
    );
    assert_eq!(
        PartHeaders::interpret([("X-Other", "ignored")]).unwrap(),
        PartHeaders::interpret(std::iter::empty::<(&str, &str)>()).unwrap()
    );
}

#[test]
fn keeps_backslashes_that_escape_nothing() {
    // Browsers send Windows paths without escaping their backslashes.
    let parsed =
        disposition(b"form-data; name=\"upload\"; filename=\"C:\\Users\\me\\a.txt\"").unwrap();
    assert_eq!(parsed.filename(), Some("C:\\Users\\me\\a.txt"));
    let parsed = disposition(b"form-data; name=\"a \\\"b\\\" \\\\ \\c\"").unwrap();
    assert_eq!(
        parsed.name(),
        Some("a \"b\" \\ \\c"),
        "only a quote or backslash is escaped"
    );
    assert_eq!(
        disposition(parsed.to_string().as_bytes()).unwrap(),
        parsed,
        "what is written parses back"
    );
    // A name ending in a backslash escapes its closing quote, as in other readers, so the value does not end where a browser meant it to.
    assert_eq!(
        disposition(b"form-data; name=\"a\\\"; filename=\"b\"")
            .unwrap_err()
            .reason(),
        "expected a parameter separator"
    );
}

#[test]
fn writes_extended_parameters_unquoted() {
    let parsed = disposition(b"attachment; filename*=UTF-8''caf%C3%A9.txt").unwrap();
    assert_eq!(parsed.parameter("filename*"), Some("UTF-8''caf%C3%A9.txt"));
    assert_eq!(
        parsed.to_string(),
        "attachment; filename*=UTF-8''caf%C3%A9.txt"
    );
    let written = ContentDisposition::new("attachment")
        .unwrap()
        .with_parameter("filename*", "UTF-8''a%20b")
        .unwrap();
    assert_eq!(written.to_string(), "attachment; filename*=UTF-8''a%20b");
    let error = ContentDisposition::new("attachment")
        .unwrap()
        .with_parameter("filename*", "not a token")
        .unwrap_err();
    assert_eq!(error.reason(), "extended parameter value is not a token");
}

#[test]
fn converts_and_writes_field_values() {
    let content_type = ContentType::parse(b"text/plain; charset=utf-8").unwrap();
    assert_eq!(content_type.to_string(), "text/plain; charset=utf-8");
    assert_eq!(MediaType::from(content_type).essence(), "text/plain");
    assert_eq!(ContentTransferEncoding::EightBit.to_string(), "8bit");
    let boundary: Boundary = "abc".parse().unwrap();
    assert_eq!(boundary.as_ref(), "abc");
    assert!("".parse::<Boundary>().is_err());
}

#[test]
fn debug_output_of_iterators_shows_no_content() {
    let body = b"--b\r\nX-Secret: hunter2\r\n\r\npassword\r\n--b--";
    let boundary = Boundary::new("b").unwrap();
    let mut iterator = parts(body, &boundary);
    assert_eq!(format!("{iterator:?}"), "Parts { position: 0, length: 41 }");
    let part = iterator.next().unwrap().unwrap();
    assert_eq!(
        format!("{:?}", part.head().fields()),
        "PartFields { remaining: <19 bytes> }"
    );
    assert!(!format!("{iterator:?}{:?}", part.head().fields()).contains("hunter2"));
}
