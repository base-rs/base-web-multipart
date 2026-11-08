// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_media_type::MediaType;
use base_web_multipart::{Boundary, BoundaryError};

#[test]
fn accepts_only_the_boundaries_rfc_2046_allows() {
    let longest = "a".repeat(Boundary::LENGTH_LIMIT);
    for valid in ["a", "simple boundary", "'()+_,-./:=?", longest.as_str()] {
        assert_eq!(Boundary::new(valid).unwrap().as_str(), valid);
    }
    let too_long = "a".repeat(Boundary::LENGTH_LIMIT + 1);
    for invalid in [
        "",
        "trailing ",
        "semi;colon",
        "quote\"",
        "line\r\nbreak",
        "caf\u{e9}",
        too_long.as_str(),
    ] {
        assert_eq!(
            Boundary::new(invalid),
            Err(BoundaryError::Invalid),
            "{invalid:?}"
        );
    }
}

#[test]
fn reads_and_writes_media_types() {
    let media_type = MediaType::parse("Multipart/Mixed; Boundary=abc").unwrap();
    assert_eq!(
        Boundary::from_media_type(&media_type).unwrap().as_str(),
        "abc"
    );
    let cases = [
        ("text/plain; boundary=abc", BoundaryError::NotMultipart),
        ("multipart/form-data", BoundaryError::Missing),
        (
            "multipart/form-data; boundary=\"trailing \"",
            BoundaryError::Invalid,
        ),
    ];
    for (text, error) in cases {
        assert_eq!(
            Boundary::from_media_type(&MediaType::parse(text).unwrap()),
            Err(error),
            "{text}"
        );
    }
    let boundary = Boundary::new("with space").unwrap();
    assert_eq!(
        boundary.to_media_type("mixed").unwrap().to_string(),
        "multipart/mixed; boundary=\"with space\""
    );
    assert!(boundary.to_media_type("*").is_err());
    let form = boundary.form_data_media_type();
    assert_eq!(
        (form.essence(), Boundary::from_media_type(&form)),
        ("multipart/form-data", Ok(boundary.clone()))
    );
    assert_eq!(
        (boundary.to_string(), format!("{boundary:?}")),
        (
            "with space".to_owned(),
            "Boundary(\"with space\")".to_owned()
        )
    );
}

#[test]
fn generates_unpredictable_boundaries() {
    let boundaries: Vec<Boundary> = (0..64).map(|_| Boundary::generate()).collect();
    for boundary in &boundaries {
        assert_eq!(boundary.as_str().len(), 32);
        assert!(
            boundary
                .as_str()
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        );
        assert_eq!(Boundary::new(boundary.as_str()).as_ref(), Ok(boundary));
    }
    let unique: std::collections::HashSet<&Boundary> = boundaries.iter().collect();
    assert_eq!(unique.len(), boundaries.len());
}

#[test]
fn errors_describe_themselves() {
    let errors = [
        (BoundaryError::NotMultipart, "media type is not multipart"),
        (
            BoundaryError::Missing,
            "multipart media type has no boundary",
        ),
        (BoundaryError::Invalid, "invalid multipart boundary"),
    ];
    for (error, description) in errors {
        assert_eq!(error.to_string(), description);
    }
    let error: Box<dyn std::error::Error> = Box::new(BoundaryError::Invalid);
    assert!(error.source().is_none());
}
