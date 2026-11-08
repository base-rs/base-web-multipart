# Releases

## v0.1.0

- Add `MultipartParser`, a sans-I/O parser for multipart bodies following RFC 2046, which is fed bytes as they arrive, reports each part's header section and content borrowed from its input, applies limits on the preamble, header sections, and number of parts, and rejects folded header lines and misplaced boundaries.
- Add `parts`, which iterates over the parts of a complete body.
- Add `MultipartWriter`, which writes a body and refuses header lines and content that would change its structure, and `Boundary`, which validates, reads, and generates boundaries.
- Add the `ContentDisposition`, `ContentType`, and `ContentTransferEncoding` field types and the `PartHeaders` section, for interpreting part headers with `base-message`.
- Add `ParameterRules`, which `ContentDisposition::name` and `filename` follow for extended parameters such as `filename*`: refused in form data, as RFC 7578 requires, and decoded and preferred in other MIME parts, as RFC 2231 does.
