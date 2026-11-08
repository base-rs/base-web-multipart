// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_multipart::{
    Boundary, ContentDisposition, MultipartWriteError, MultipartWriter, parts,
};

#[test]
fn written_bodies_parse_back() {
    let boundary = Boundary::generate();
    let mut writer = MultipartWriter::new(boundary.clone());
    assert_eq!(writer.boundary(), &boundary);
    let mut body = Vec::new();
    let file = ContentDisposition::form_data("file")
        .unwrap()
        .with_parameter("filename", "caf\u{e9}.txt")
        .unwrap()
        .to_string();
    writer
        .start_part(
            [
                ("Content-Disposition", file.as_bytes()),
                ("Content-Type", b"text/plain"),
            ],
            &mut body,
        )
        .unwrap();
    for chunk in [&b"first line\r\n"[..], b"--", b"almost a boundary\r\n-"] {
        writer.write_content(chunk, &mut body).unwrap();
    }
    writer
        .start_part(
            [("Content-Disposition", "form-data; name=\"empty\"")],
            &mut body,
        )
        .unwrap();
    writer
        .start_part(std::iter::empty::<(&str, &[u8])>(), &mut body)
        .unwrap();
    writer.write_content(b"last", &mut body).unwrap();
    writer.clone().finish(&mut Vec::new());
    writer.finish(&mut body);
    let parsed: Vec<_> = parts(&body, &boundary).map(Result::unwrap).collect();
    assert_eq!(parsed.len(), 3);
    assert_eq!(
        parsed[0].content(),
        b"first line\r\n--almost a boundary\r\n-"
    );
    assert_eq!(
        parsed[0]
            .head()
            .fields()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["Content-Disposition", "Content-Type"]
    );
    assert_eq!(
        (parsed[1].content(), parsed[2].content()),
        (&b""[..], &b"last"[..])
    );
    assert!(parsed[2].head().is_empty());
}

#[test]
fn a_body_without_parts_is_its_closing_boundary() {
    let mut body = Vec::new();
    MultipartWriter::new(Boundary::new("b").unwrap()).finish(&mut body);
    assert_eq!(body, b"--b--\r\n");
    assert_eq!(parts(&body, &Boundary::new("b").unwrap()).count(), 0);
}

#[test]
fn refuses_fields_that_would_change_the_structure_and_writes_nothing() {
    let mut writer = MultipartWriter::new(Boundary::new("b").unwrap());
    let mut body = b"kept".to_vec();
    let cases: [(&str, &[u8], MultipartWriteError); 6] = [
        ("", b"x", MultipartWriteError::InvalidFieldName),
        ("Bad Name", b"x", MultipartWriteError::InvalidFieldName),
        ("Name:", b"x", MultipartWriteError::InvalidFieldName),
        (
            "X-Injected",
            b"a\r\nContent-Type: text/html",
            MultipartWriteError::InvalidFieldValue,
        ),
        ("X-Nul", b"a\x00", MultipartWriteError::InvalidFieldValue),
        ("--b", b"x", MultipartWriteError::Delimiter),
    ];
    for (name, value, error) in cases {
        assert_eq!(
            writer.start_part([("X-Valid", &b"1"[..]), (name, value)], &mut body),
            Err(error),
            "{name:?}"
        );
        assert_eq!(body, b"kept");
    }
    assert_eq!(
        writer.write_content(b"x", &mut body),
        Err(MultipartWriteError::NoPart),
        "a failed part did not start"
    );
    writer
        .start_part([("X-Value", "caf\u{e9}\twith tab")], &mut body)
        .unwrap();
    assert!(body.ends_with(b"X-Value: caf\xc3\xa9\twith tab\r\n\r\n"));
}

#[test]
fn refuses_content_containing_the_delimiter_however_it_is_split() {
    let boundary = Boundary::new("boundary").unwrap();
    let content = b"before\r\n--boundary after";
    for split in 0..=content.len() {
        let mut writer = MultipartWriter::new(boundary.clone());
        let mut body = Vec::new();
        writer.start_part([("A", "1")], &mut body).unwrap();
        let (first, second) = content.split_at(split);
        let results = [
            writer.write_content(first, &mut body),
            writer.write_content(second, &mut body),
        ];
        assert!(
            results.contains(&Err(MultipartWriteError::Delimiter)),
            "split at {split}"
        );
    }
    let mut writer = MultipartWriter::new(boundary.clone());
    let mut body = Vec::new();
    writer.start_part([("A", "1")], &mut body).unwrap();
    let length = body.len();
    assert_eq!(
        writer.write_content(b"--boundary", &mut body),
        Err(MultipartWriteError::Delimiter),
        "content follows a line break"
    );
    assert_eq!(body.len(), length, "nothing is written");
    writer.write_content(b"--bound", &mut body).unwrap();
    writer.write_content(b"ar", &mut body).unwrap();
    writer.write_content(b"x\r", &mut body).unwrap();
    assert_eq!(
        writer.write_content(b"\n--boundary", &mut body),
        Err(MultipartWriteError::Delimiter)
    );
    writer.write_content(b"\n--boundar", &mut body).unwrap();
    writer.write_content(b"", &mut body).unwrap();
    writer.write_content(b"X", &mut body).unwrap();
    writer.finish(&mut body);
    let parsed: Vec<_> = parts(&body, &boundary).map(Result::unwrap).collect();
    assert_eq!(parsed[0].content(), b"--boundarx\r\n--boundarX");
}

#[test]
fn errors_describe_themselves() {
    let errors = [
        (
            MultipartWriteError::InvalidFieldName,
            "invalid multipart part header field name",
        ),
        (
            MultipartWriteError::InvalidFieldValue,
            "invalid multipart part header field value",
        ),
        (
            MultipartWriteError::Delimiter,
            "multipart part contains its boundary delimiter",
        ),
        (
            MultipartWriteError::NoPart,
            "multipart content written before any part",
        ),
    ];
    for (error, description) in errors {
        assert_eq!(error.to_string(), description);
    }
    let error: Box<dyn std::error::Error> = Box::new(MultipartWriteError::NoPart);
    assert!(error.source().is_none());
    assert!(
        format!("{:?}", MultipartWriter::new(Boundary::new("b").unwrap()))
            .contains("MultipartWriter")
    );
}
