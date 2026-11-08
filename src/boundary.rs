// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_media_type::{MediaType, MediaTypeError};
use std::fmt;

/// A multipart boundary, as RFC 2046 section 5.1.1 defines it: 1 to 70 characters from a restricted set, not ending in a space.
///
/// ```
/// use base_web_multipart::Boundary;
/// use base_web_media_type::MediaType;
///
/// let media_type = MediaType::parse("multipart/form-data; boundary=\"simple boundary\"")?;
/// let boundary = Boundary::from_media_type(&media_type)?;
/// assert_eq!(boundary.as_str(), "simple boundary");
///
/// let generated = Boundary::generate();
/// assert_eq!(generated.as_str().len(), 32);
/// assert_eq!(generated.form_data_media_type().parameter("boundary"), Some(generated.as_str()));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Boundary(String);

/// Why a boundary was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BoundaryError {
    /// The media type is not a `multipart` type.
    NotMultipart,
    /// The media type has no `boundary` parameter.
    Missing,
    /// The boundary is empty, longer than 70 characters, ends in a space, or has a character RFC 2046 does not allow.
    Invalid,
}

impl Boundary {
    /// The longest boundary RFC 2046 allows.
    pub const LENGTH_LIMIT: usize = 70;

    /// The random bytes in a generated boundary: 192 bits, written as 32 characters.
    const RANDOM_BYTES: usize = 24;

    /// A boundary from text.
    ///
    /// # Errors
    ///
    /// [`BoundaryError::Invalid`] when the text is not a boundary RFC 2046 allows.
    pub fn new(text: &str) -> Result<Self, BoundaryError> {
        let allowed = |byte: u8| byte.is_ascii_alphanumeric() || b"'()+_,-./:=? ".contains(&byte);
        if text.is_empty()
            || text.len() > Self::LENGTH_LIMIT
            || text.ends_with(' ')
            || !text.bytes().all(allowed)
        {
            return Err(BoundaryError::Invalid);
        }
        Ok(Self(text.to_owned()))
    }

    /// The boundary of a `multipart` media type, from its `boundary` parameter.
    ///
    /// # Errors
    ///
    /// [`BoundaryError::NotMultipart`] for another type, [`BoundaryError::Missing`] when there is no `boundary` parameter, and [`BoundaryError::Invalid`] when it is not a valid boundary.
    pub fn from_media_type(media_type: &MediaType) -> Result<Self, BoundaryError> {
        if media_type.top_level_type() != "multipart" {
            return Err(BoundaryError::NotMultipart);
        }
        Self::new(
            media_type
                .parameter("boundary")
                .ok_or(BoundaryError::Missing)?,
        )
    }

    /// A new boundary of 192 random bits from the operating system, written as 32 URL-safe base64 characters, so that content cannot contain it by chance or by an attacker's design.
    ///
    /// # Panics
    ///
    /// When the operating system's random number generator fails, which it does not on supported platforms; the standard library's hash maps rely on it in the same way.
    pub fn generate() -> Self {
        let mut bytes = [0; Self::RANDOM_BYTES];
        getrandom::fill(&mut bytes).expect("the operating system provides random bytes");
        Self(base64url(&bytes))
    }

    /// The boundary as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The media type `multipart/{subtype}` with this boundary as its parameter, such as `multipart/mixed; boundary=...`.
    ///
    /// # Errors
    ///
    /// When the subtype is not a token, or is a wildcard.
    pub fn to_media_type(&self, subtype: &str) -> Result<MediaType, MediaTypeError> {
        MediaType::new("multipart", subtype)?.with_parameter("boundary", &self.0)
    }

    /// The media type `multipart/form-data` with this boundary as its parameter.
    pub fn form_data_media_type(&self) -> MediaType {
        MediaType::MULTIPART_FORM_DATA
            .with_parameter("boundary", &self.0)
            .expect("a valid boundary is a valid parameter value")
    }
}

impl std::str::FromStr for Boundary {
    type Err = BoundaryError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::new(text)
    }
}

impl AsRef<str> for Boundary {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Boundary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("Boundary").field(&self.0).finish()
    }
}

impl fmt::Display for Boundary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for BoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotMultipart => "media type is not multipart",
            Self::Missing => "multipart media type has no boundary",
            Self::Invalid => "invalid multipart boundary",
        })
    }
}

impl std::error::Error for BoundaryError {}

/// URL-safe base64 without padding, of a length divisible by 3.
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut text = String::with_capacity(bytes.len() / 3 * 4);
    for chunk in bytes.chunks_exact(3) {
        let triple = u32::from(chunk[0]) << 16 | u32::from(chunk[1]) << 8 | u32::from(chunk[2]);
        for shift in [18, 12, 6, 0] {
            text.push(char::from(ALPHABET[(triple >> shift) as usize & 63]));
        }
    }
    text
}
