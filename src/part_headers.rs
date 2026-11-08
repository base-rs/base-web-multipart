// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{ContentDisposition, ContentTransferEncoding, ContentType};
use base_message::FieldSection;

/// The header fields of a part that this crate understands, as a [`FieldSection`] to interpret a [`PartHead`](crate::PartHead) with. Other fields are ignored.
///
/// Each is a singleton, so a second `Content-Type` or `Content-Disposition` is an error rather than a choice between them. A transfer encoding other than an identity one is an invalid value, since the content would otherwise be misread. Flatten this section into one of your own to read other fields too.
///
/// ```
/// use base_web_multipart::{PartHead, PartHeaders};
/// use base_message::FieldSection;
///
/// let headers = PartHeaders::interpret([("Content-Disposition", "form-data; name=\"a\""), ("Content-Type", "text/plain")])?;
/// assert_eq!(headers.content_disposition.as_ref().and_then(|disposition| disposition.name()), Some("a"));
/// assert!(PartHeaders::interpret([("Content-Transfer-Encoding", "base64")]).is_err());
/// # Ok::<(), base_message::FieldError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, FieldSection)]
#[non_exhaustive]
pub struct PartHeaders<'a> {
    /// The part's media type, if it has one.
    #[field("content-type")]
    pub content_type: Option<ContentType>,
    /// The part's disposition, such as a form field's name and a file's name.
    #[field("content-disposition")]
    pub content_disposition: Option<ContentDisposition<'a>>,
    /// The part's transfer encoding, which can only be an identity encoding.
    #[field("content-transfer-encoding")]
    pub content_transfer_encoding: Option<ContentTransferEncoding>,
}
