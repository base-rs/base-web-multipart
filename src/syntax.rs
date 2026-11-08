// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Byte classes and the parameterized-value grammar shared by the parser, the writer, and the field types.

use base_message::FieldValueError;
use std::borrow::Cow;

/// A byte of a field name, RFC 5322's `ftext`: visible ASCII except the colon.
pub(crate) fn is_field_name_byte(byte: u8) -> bool {
    matches!(byte, 33..=57 | 59..=126)
}

/// A byte of a field value: visible ASCII, space, horizontal tab, or a byte above ASCII, which RFC 7578 allows for names and filenames in UTF-8. Other controls, including CR and LF, are not.
pub(crate) fn is_field_value_byte(byte: u8) -> bool {
    byte == b'\t' || (byte >= 0x20 && byte != 0x7f)
}

/// A byte of an RFC 9110 token.
pub(crate) fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

/// Whether text is a non-empty token.
pub(crate) fn is_token(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(is_token_byte)
}

/// A field value as text: UTF-8 without controls other than horizontal tab.
pub(crate) fn field_text(value: &[u8]) -> Result<&str, FieldValueError> {
    let text =
        std::str::from_utf8(value).map_err(|_| FieldValueError::new("value is not UTF-8"))?;
    if !value.iter().all(|&byte| is_field_value_byte(byte)) {
        return Err(FieldValueError::new("value contains a control character"));
    }
    Ok(text)
}

/// Text lowercased only when it has uppercase letters, so that most names are borrowed.
pub(crate) fn lowercase(text: &str) -> Cow<'_, str> {
    if text.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(text.to_ascii_lowercase())
    } else {
        Cow::Borrowed(text)
    }
}

/// The parameters a parameterized value may have, so that a hostile value cannot make the parameter list large.
pub(crate) const PARAMETER_COUNT_LIMIT: usize = 64;

/// A value and its parameters, such as `form-data; name="avatar"`.
pub(crate) type Parameterized<'a> = (Cow<'a, str>, Vec<(Cow<'a, str>, Cow<'a, str>)>);

/// Parse `token *( OWS ";" OWS name OWS "=" OWS ( token / quoted-string ) ) OWS`, as RFC 2183 and RFC 2045 write parameterized values, with optional whitespace around the separators, which MIME allows. The value and parameter names are lowercased, and a parameter given twice is an error.
pub(crate) fn parse_parameterized(text: &str) -> Result<Parameterized<'_>, FieldValueError> {
    let mut rest = text.trim_matches([' ', '\t']);
    let end = rest
        .find(|character: char| !character.is_ascii() || !is_token_byte(character as u8))
        .unwrap_or(rest.len());
    if end == 0 {
        return Err(FieldValueError::new("missing value"));
    }
    let value = lowercase(&rest[..end]);
    rest = &rest[end..];
    let mut parameters: Vec<(Cow<'_, str>, Cow<'_, str>)> = Vec::new();
    loop {
        rest = rest.trim_start_matches([' ', '\t']);
        if rest.is_empty() {
            return Ok((value, parameters));
        }
        rest = rest
            .strip_prefix(';')
            .ok_or(FieldValueError::new("expected a parameter separator"))?
            .trim_start_matches([' ', '\t']);
        let end = rest
            .find(|character: char| !character.is_ascii() || !is_token_byte(character as u8))
            .unwrap_or(rest.len());
        if end == 0 {
            return Err(FieldValueError::new("missing parameter name"));
        }
        let name = lowercase(&rest[..end]);
        rest = rest[end..]
            .trim_start_matches([' ', '\t'])
            .strip_prefix('=')
            .ok_or(FieldValueError::new("expected a parameter value"))?
            .trim_start_matches([' ', '\t']);
        let (parameter, remaining) = parameter_value(rest)?;
        rest = remaining;
        if parameters.iter().any(|(existing, _)| *existing == name) {
            return Err(FieldValueError::new("duplicate parameter"));
        }
        if parameters.len() == PARAMETER_COUNT_LIMIT {
            return Err(FieldValueError::new("too many parameters"));
        }
        parameters.push((name, parameter));
    }
}

/// A token or a quoted string at the start of `text`, unescaped, and the text after it.
fn parameter_value(text: &str) -> Result<(Cow<'_, str>, &str), FieldValueError> {
    let Some(quoted) = text.strip_prefix('"') else {
        let end = text
            .find(|character: char| !character.is_ascii() || !is_token_byte(character as u8))
            .unwrap_or(text.len());
        if end == 0 {
            return Err(FieldValueError::new("missing parameter value"));
        }
        return Ok((Cow::Borrowed(&text[..end]), &text[end..]));
    };
    let mut unescaped: Option<String> = None;
    let mut characters = quoted.char_indices();
    while let Some((index, character)) = characters.next() {
        match character {
            '"' => {
                let value = unescaped.map_or(Cow::Borrowed(&quoted[..index]), Cow::Owned);
                return Ok((value, &quoted[index + 1..]));
            }
            // A backslash escapes only a quote or another backslash. Browsers do not escape backslashes in names and filenames, so any other is kept, as in a Windows path such as `C:\Users`.
            '\\' if matches!(quoted[index + 1..].chars().next(), Some('"' | '\\')) => {
                let (_, escaped) = characters
                    .next()
                    .expect("the escaped character was just seen");
                unescaped
                    .get_or_insert_with(|| quoted[..index].to_owned())
                    .push(escaped);
            }
            _ => {
                if let Some(unescaped) = &mut unescaped {
                    unescaped.push(character);
                }
            }
        }
    }
    Err(FieldValueError::new("unterminated quoted string"))
}

/// Write a parameter value: as a token when its name ends with `*`, since RFC 8187 extended values are never quoted, and otherwise as a quoted string, escaping quotes and backslashes.
pub(crate) fn write_parameter_value(output: &mut String, name: &str, value: &str) {
    if name.ends_with('*') {
        output.push_str(value);
        return;
    }
    output.push('"');
    for character in value.chars() {
        if matches!(character, '"' | '\\') {
            output.push('\\');
        }
        output.push(character);
    }
    output.push('"');
}
