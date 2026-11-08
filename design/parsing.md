# Parsing

## Requirement

`MultipartParser` must parse a body fed in pieces of any size and give the same parts, header sections, and content however the body is split. The caller owns the buffer: each call reports the bytes it consumed and one event, and the next call's input begins with the bytes not consumed. Events must borrow from the input, so that content and header fields are never copied, and a part's header section must be reported whole. The parser must need at most `minimum_buffer_size()` bytes of input to make progress or report an error, and must not search the same bytes more than a bounded number of times however the input is split.

Parsing must be strict wherever leniency would let two readers see different parts:

- lines end with CRLF, and a bare CR or LF in a header line is an error;
- a folded header line, one beginning with a space or tab, is an error;
- a field name is RFC 5322 `ftext`, and a field value has no control character other than horizontal tab, though bytes above ASCII are allowed for UTF-8 names and filenames;
- a boundary is followed only by optional spaces and tabs and CRLF, or by `--`; anything else, including the boundary delimiter appearing within content, is an error;
- the body may begin with the boundary itself, and anything after the closing boundary is ignored.

The parser must apply limits on the preamble's size, each header section's size, including any padding after the boundary before it, each header section's number of fields, and the number of parts, with defaults of 64 KiB, 64 KiB, 64, and 128. It must not limit content. Errors must give the offset in the body, and errors and `Debug` output must never include content or field values.

`ContentDisposition` must parse a disposition type and parameters with token or quoted-string values, lowercasing the type and parameter names, rejecting duplicate parameters, more than 64 parameters, invalid UTF-8, and control characters, and borrowing unless a quoted string has escapes. `ContentTransferEncoding` must accept only the identity encodings `7bit`, `8bit`, and `binary`.

## Rationale

The user asked for a strict, sans-I/O parser fed bytes, with the Ruby gem's structural limits, that rejects folded header lines; an option to unfold them was not added, since unfolding changes a value, which then cannot be borrowed, and RFC 7578 senders do not fold. Multipart parsers that disagree about where a part ends are how a malicious upload is read as extra form fields, so every ambiguity is an error rather than a guess. The caller-owned buffer, borrowed events, and whole header sections were chosen in the initial implementation, so that part headers can be interpreted by `base-message` sections that borrow from them, and uploads pass through without a copy. Accepting only identity transfer encodings was also chosen then: this crate does not decode content, and ignoring an encoding such as `base64` would misread the content.

## Scope

`MultipartParser`, `MultipartEvent`, `Parsed`, `MultipartLimits`, `MultipartError`, `PartHead`, `Parts`, `ContentDisposition`, `ContentType`, `ContentTransferEncoding`, and `PartHeaders`.

## Validation

`tests/multipart_parser.rs` parses bodies split at every point and fed a byte at a time and compares them with the whole body parsed at once, and covers each strictness rule, limit, and error offset. `tests/round_trip.rs` parses written bodies fed in random pieces. `tests/field_types.rs` covers the field types and `PartHeaders`.
