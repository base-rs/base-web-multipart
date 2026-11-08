# Scope

## Requirement

`base-web-multipart` must hold the framing of MIME multipart bodies, as RFC 2046 section 5.1 defines it: boundaries, parsing a body into parts with their header sections and content, and writing one. It must also hold the part header fields RFC 2046 and RFC 7578 give meaning to, `Content-Disposition`, `Content-Type`, and `Content-Transfer-Encoding`, as `base-message` field types, with a section that interprets them.

The crate must do no I/O: the parser is fed bytes and the writer appends bytes to a caller's buffer. It must not decode content, interpret forms, or limit content: form fields, uploads, and their limits belong to `base-web-content`, and transport to `base-http`. It parses part header sections itself, since their syntax differs from HTTP's, and interprets them only through `base-message`.

Its normal dependencies must be only `base-message`, `base-web-media-type`, `getrandom`, and `memchr`.

## Rationale

The user asked to port the Ruby `protocol-multipart` gem as a sans-I/O crate fed bytes, covering RFC 2046 parts, RFC 7578 metadata, structural limits, and a writer, with boundaries from `getrandom`, and with forms, their limits, and saving uploads in a separate `base-web-content`. The user decided that each protocol crate parses its own field sections and passes them to `base-message` as name and value pairs, so that part headers are interpreted the same way as HTTP's. `memchr` was chosen in the initial implementation for searching content for the boundary delimiter, which is the hot path for uploads.

## Scope

The `base-web-multipart` crate as a whole.

## Validation

Inspect `cargo tree -e normal --depth 1`, and review new public items against this scope: anything that reads forms, decodes transfer encodings, or performs I/O belongs elsewhere.
