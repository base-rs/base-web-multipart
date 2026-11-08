# Writing

## Requirement

`MultipartWriter` must write a body that any RFC 2046 reader parses into exactly the parts written. It must refuse, writing nothing and leaving its state unchanged:

- a field name that is not RFC 5322 `ftext`, and a field value with a control character other than horizontal tab, so that no header line can be injected;
- a header line that begins with the boundary, which would form the delimiter after the preceding line break;
- content that contains the boundary after a line break, whether CRLF or a bare CR or LF, including content that begins with the boundary after the header section's line break and a boundary split across calls.

`Boundary::generate` must draw 192 bits from the operating system's random number generator. A body with no parts must be written as its closing boundary alone. The writer does not apply a reader's limits, such as on the number of parts, which its documentation must say.

## Rationale

The user asked for a writer, with boundaries from `getrandom`. A boundary an attacker cannot predict keeps uploaded content from containing it, but the writer also checks content, so a caller's own boundary cannot be broken by content either. Refusing invalid fields rather than escaping them keeps what was asked for and what was written the same.

## Scope

`MultipartWriter`, `MultipartWriteError`, `Boundary`, and the `Display` of `ContentDisposition`.

## Validation

`tests/multipart_writer.rs` covers each refusal, delimiters split at every point, and round trips. `tests/round_trip.rs` writes random bodies whose content is biased toward delimiters, parses them back in random pieces, and requires the writer to have refused some content.
