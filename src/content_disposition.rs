// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::syntax::{
    PARAMETER_COUNT_LIMIT, field_text, is_field_value_byte, is_token, lowercase,
    parse_parameterized, write_parameter_value,
};
use base_message::{FieldType, FieldValueError, combination::Singleton};
use std::borrow::Cow;
use std::fmt;

/// A `Content-Disposition` field, as RFC 2183 defines it for MIME parts and RFC 7578 uses for form data: a disposition type, such as `form-data`, and parameters, such as `name` and `filename`.
///
/// Parsing borrows from the field value, copying only a quoted string with escapes. Names and filenames are returned as they were sent: RFC 7578 lets them be UTF-8, and browsers percent-encode the quotes and line breaks in them, which this type does not decode. In a quoted string, a backslash escapes only a quote or another backslash; any other backslash is kept, since browsers send Windows paths unescaped. The extended `filename*` parameter of RFC 8187 is an ordinary parameter here, not decoded, and is written unquoted.
///
/// A value that is not UTF-8, as a form on a page in a legacy character set may send, is invalid, as are an empty parameter, such as a trailing `;`, and a value outside quotes that is not a token.
///
/// ```
/// use base_web_multipart::ContentDisposition;
/// use base_message::FieldType;
///
/// let disposition = ContentDisposition::parse(b"form-data; name=\"avatar\"; filename=\"caf\xc3\xa9.png\"")?;
/// assert_eq!(disposition.disposition_type(), "form-data");
/// assert_eq!((disposition.name(), disposition.filename()), (Some("avatar"), Some("café.png")));
///
/// let written = ContentDisposition::form_data("note")?.with_parameter("filename", "a \"quoted\" name")?;
/// assert_eq!(written.to_string(), "form-data; name=\"note\"; filename=\"a \\\"quoted\\\" name\"");
/// # Ok::<(), base_message::FieldValueError>(())
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ContentDisposition<'a> {
    disposition_type: Cow<'a, str>,
    parameters: Vec<(Cow<'a, str>, Cow<'a, str>)>,
}

impl ContentDisposition<'static> {
    /// A disposition without parameters, such as `attachment`. The type is lowercased.
    ///
    /// # Errors
    ///
    /// When the type is not a token.
    pub fn new(disposition_type: &str) -> Result<Self, FieldValueError> {
        if !is_token(disposition_type) {
            return Err(FieldValueError::new("disposition type is not a token"));
        }
        Ok(ContentDisposition {
            disposition_type: Cow::Owned(disposition_type.to_ascii_lowercase()),
            parameters: Vec::new(),
        })
    }

    /// A `form-data` disposition for a form field with this name, as RFC 7578 section 4.2 requires of each part.
    ///
    /// # Errors
    ///
    /// When the name contains a control character, such as CR or LF, which a quoted string cannot carry.
    pub fn form_data(name: &str) -> Result<Self, FieldValueError> {
        ContentDisposition {
            disposition_type: Cow::Borrowed("form-data"),
            parameters: Vec::new(),
        }
        .with_parameter("name", name)
    }
}

impl<'a> ContentDisposition<'a> {
    /// The disposition with a parameter added. The name is lowercased.
    ///
    /// # Errors
    ///
    /// When the name is not a token or is already present, the value contains a control character other than horizontal tab, the name ends with `*`, marking an RFC 8187 extended value, and the value is not a token, or the disposition already has the most parameters it may have.
    pub fn with_parameter(
        mut self,
        name: &str,
        value: &str,
    ) -> Result<ContentDisposition<'a>, FieldValueError> {
        if !is_token(name) {
            return Err(FieldValueError::new("parameter name is not a token"));
        }
        if !value.bytes().all(is_field_value_byte) {
            return Err(FieldValueError::new(
                "parameter value contains a control character",
            ));
        }
        if name.ends_with('*') && !is_token(value) {
            return Err(FieldValueError::new(
                "extended parameter value is not a token",
            ));
        }
        let name = lowercase(name).into_owned();
        if self.parameter(&name).is_some() {
            return Err(FieldValueError::new("duplicate parameter"));
        }
        if self.parameters.len() == PARAMETER_COUNT_LIMIT {
            return Err(FieldValueError::new("too many parameters"));
        }
        self.parameters
            .push((Cow::Owned(name), Cow::Owned(value.to_owned())));
        Ok(self)
    }

    /// The lowercase disposition type, such as `form-data`, `attachment`, or `inline`.
    pub fn disposition_type(&self) -> &str {
        &self.disposition_type
    }

    /// The value of a parameter. Names are compared case-insensitively.
    pub fn parameter(&self, name: &str) -> Option<&str> {
        self.parameters
            .iter()
            .find(|(parameter, _)| parameter.eq_ignore_ascii_case(name))
            .map(|(_, value)| &**value)
    }

    /// The lowercase parameter names and their values, in order.
    pub fn parameters(&self) -> impl ExactSizeIterator<Item = (&str, &str)> {
        self.parameters
            .iter()
            .map(|(name, value)| (&**name, &**value))
    }

    /// The `name` parameter, a form field's name.
    pub fn name(&self) -> Option<&str> {
        self.parameter("name")
    }

    /// The `filename` parameter, a file's name as the sender gave it, which must not be used as a local path without checking it.
    pub fn filename(&self) -> Option<&str> {
        self.parameter("filename")
    }

    /// The disposition, owning its text.
    pub fn into_owned(self) -> ContentDisposition<'static> {
        let owned = |text: Cow<'a, str>| Cow::Owned(text.into_owned());
        ContentDisposition {
            disposition_type: owned(self.disposition_type),
            parameters: self
                .parameters
                .into_iter()
                .map(|(name, value)| (owned(name), owned(value)))
                .collect(),
        }
    }
}

impl<'a> FieldType<'a> for ContentDisposition<'a> {
    type Combination = Singleton;

    fn parse(value: &'a [u8]) -> Result<Self, FieldValueError> {
        let (disposition_type, parameters) = parse_parameterized(field_text(value)?)?;
        Ok(Self {
            disposition_type,
            parameters,
        })
    }
}

/// Writes the disposition as a field value, with every parameter value quoted except extended values.
impl fmt::Display for ContentDisposition<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = String::from(&*self.disposition_type);
        for (name, value) in &self.parameters {
            text.push_str("; ");
            text.push_str(name);
            text.push('=');
            write_parameter_value(&mut text, name, value);
        }
        formatter.write_str(&text)
    }
}

/// Shows the disposition type and parameter names, and the lengths of parameter values, since a form field's name or a filename can identify a person.
impl fmt::Debug for ContentDisposition<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parameters: Vec<_> = self
            .parameters
            .iter()
            .map(|(name, value)| (&**name, format!("<{} bytes>", value.len())))
            .collect();
        formatter
            .debug_struct("ContentDisposition")
            .field("disposition_type", &self.disposition_type)
            .field("parameters", &parameters)
            .finish()
    }
}
