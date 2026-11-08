// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_message::{FieldType, FieldValueError, ascii_text, combination::Singleton};
use base_web_media_type::MediaType;

/// A part's `Content-Type` field: its media type, which RFC 2046 defaults to `text/plain; charset=us-ascii` and RFC 7578 to `text/plain` for form data when the field is absent.
///
/// ```
/// use base_web_multipart::ContentType;
/// use base_message::FieldType;
/// use base_web_media_type::MediaType;
///
/// let content_type = ContentType::parse(b"image/png")?;
/// assert_eq!(content_type.media_type(), &MediaType::IMAGE_PNG);
/// # Ok::<(), base_message::FieldValueError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentType(MediaType);

impl ContentType {
    /// The media type.
    pub fn media_type(&self) -> &MediaType {
        &self.0
    }

    /// The media type, taken from the field.
    pub fn into_media_type(self) -> MediaType {
        self.0
    }
}

impl From<MediaType> for ContentType {
    fn from(media_type: MediaType) -> Self {
        Self(media_type)
    }
}

impl FieldType<'_> for ContentType {
    type Combination = Singleton;

    fn parse(value: &[u8]) -> Result<Self, FieldValueError> {
        MediaType::parse(ascii_text(value)?)
            .map(Self)
            .map_err(|_| FieldValueError::new("invalid media type"))
    }
}
