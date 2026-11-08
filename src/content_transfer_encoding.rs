// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_message::{FieldType, FieldValueError, ascii_text, combination::Singleton};

/// A part's `Content-Transfer-Encoding` field, limited to the identity encodings of RFC 2045 section 6, under which content is passed as it was written.
///
/// This crate does not decode content, so `base64` and `quoted-printable`, and any other encoding, are invalid values: content in them would otherwise be read as if it were not encoded. RFC 7578 section 4.7 deprecates the field for form data.
///
/// ```
/// use base_web_multipart::ContentTransferEncoding;
/// use base_message::FieldType;
///
/// assert_eq!(ContentTransferEncoding::parse(b"Binary"), Ok(ContentTransferEncoding::Binary));
/// assert!(ContentTransferEncoding::parse(b"base64").is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContentTransferEncoding {
    /// `7bit`: lines of ASCII.
    SevenBit,
    /// `8bit`: lines that may have bytes above ASCII.
    EightBit,
    /// `binary`: any bytes.
    Binary,
}

impl ContentTransferEncoding {
    /// The encoding's name, as it is written.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SevenBit => "7bit",
            Self::EightBit => "8bit",
            Self::Binary => "binary",
        }
    }
}

impl std::fmt::Display for ContentTransferEncoding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FieldType<'_> for ContentTransferEncoding {
    type Combination = Singleton;

    fn parse(value: &[u8]) -> Result<Self, FieldValueError> {
        let text = ascii_text(value)?;
        [Self::SevenBit, Self::EightBit, Self::Binary]
            .into_iter()
            .find(|encoding| encoding.as_str().eq_ignore_ascii_case(text))
            .ok_or(FieldValueError::new(
                "unsupported content transfer encoding",
            ))
    }
}
