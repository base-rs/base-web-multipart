// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_media_type::MediaType;

/// The rules for a `Content-Disposition` field's extended parameters, such as `filename*`, when reading its name or filename. Forms and other MIME parts follow different RFCs, so a disposition does not choose for you.
///
/// ```
/// use base_web_multipart::{ContentDisposition, ParameterRules};
/// use base_web_media_type::MediaType;
/// use base_message::FieldType;
///
/// let disposition = ContentDisposition::parse(b"attachment; filename=\"EURO rates\"; filename*=utf-8''%e2%82%ac%20rates")?;
/// assert_eq!(disposition.filename(ParameterRules::Mime)?.as_deref(), Some("€ rates"));
/// assert!(disposition.filename(ParameterRules::FormData).is_err());
///
/// let rules = ParameterRules::for_media_type(&MediaType::parse("multipart/form-data; boundary=x")?);
/// assert_eq!(rules, ParameterRules::FormData);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParameterRules {
    /// RFC 7578, for `multipart/form-data`: extended parameters must not be used, so a disposition with any, including a continuation such as `filename*0`, is an error. Readers disagree about which of `filename` and `filename*` applies, so refusing both keeps them from seeing different names.
    FormData,
    /// RFC 2231, for other MIME parts, such as an email's attachments: an extended parameter, such as `filename*`, is decoded and takes the place of the plain one. Its character set must be UTF-8 or US-ASCII, and its percent-encoding valid; the language is ignored. Continuations, such as `filename*0`, are not supported yet and are an error.
    Mime,
}

impl ParameterRules {
    /// The rules for a body of this media type: `FormData` for `multipart/form-data`, and `Mime` for any other.
    pub fn for_media_type(media_type: &MediaType) -> Self {
        if media_type
            .essence()
            .eq_ignore_ascii_case("multipart/form-data")
        {
            Self::FormData
        } else {
            Self::Mime
        }
    }
}
