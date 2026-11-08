# `base-web-multipart`

MIME multipart bodies, parsed and written without I/O, following RFC 2046 and RFC 7578, for Rust.

`MultipartParser` is fed bytes as they arrive and reports each part's header section and content as slices of your buffer, never copying them, with limits on the structure a hostile body could inflate. Part headers are interpreted with [`base-message`](https://github.com/base-rs/base-message), and `ContentDisposition` reads form field names and filenames. `MultipartWriter` writes a body, refusing header lines and content that would change its structure, with boundaries generated from 192 random bits.

## Usage

```rust
use base_message::FieldSection;
use base_web_multipart::{Boundary, PartHeaders, parts};

let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHello, world\r\n--XyZ--\r\n";
let boundary = Boundary::new("XyZ")?;
for part in parts(body, &boundary) {
    let part = part?;
    let headers = PartHeaders::interpret(part.head())?;
    let name = headers.content_disposition.as_ref().and_then(|disposition| disposition.name());
    assert_eq!((name, part.content()), (Some("title"), &b"Hello, world"[..]));
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

See the [Getting Started](context/getting-started.md) guide for parsing a body as it arrives, limits, and writing.

## Releases

See [releases.md](releases.md).

## See Also

- [Protocol::Multipart](https://github.com/socketry/protocol-multipart), the Ruby gem this crate follows.
- [RFC 2046 section 5.1](https://www.rfc-editor.org/rfc/rfc2046#section-5.1), multipart media types, and [RFC 7578](https://www.rfc-editor.org/rfc/rfc7578), `multipart/form-data`.

## Contributing

Please open an issue or pull request on [GitHub](https://github.com/base-rs/base-web-multipart).

Install the [Bake launcher](https://github.com/base-rs/bake) and run `cargo bake test`. Use `cargo bake test:coverage --all-targets true` for the shared source-region coverage gate.

### Agent Context

Run `cargo bake agent:context:install` to install shared context and skills. Use `cargo bake agent:context:list` and `cargo bake agent:context:show` to discover relevant guides, follow repository guidance in `.agents/`, and apply skills under `.agents/skills/`.

Before planning or changing behavior, discover and read applicable `design/*.md`. Use `base-design-review` to assess plans and changes, and `base-design-author` when user requirements are introduced, changed, or removed. Review design compliance before opening or updating a pull request.
