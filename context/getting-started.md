# Getting Started

This guide explains how to parse and write multipart bodies with `base-web-multipart`.

## Multipart bodies

A multipart body, as RFC 2046 section 5.1 defines it, is a sequence of parts separated by a boundary, a line of the form `--boundary`, and ended by `--boundary--`. Each part has a header section, like a small message's, and content. HTML forms submit `multipart/form-data` bodies, as RFC 7578 describes, with one part for each field or file.

The boundary comes from the body's media type, such as `multipart/form-data; boundary=XyZ`:

```rust
use base_web_media_type::MediaType;
use base_web_multipart::Boundary;

let media_type = MediaType::parse("multipart/form-data; boundary=XyZ")?;
let boundary = Boundary::from_media_type(&media_type)?;
assert_eq!(boundary.as_str(), "XyZ");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Parsing a body as it arrives

`MultipartParser` does no I/O. Feed it the bytes you have; it reports how many it consumed and what they held. Drop the consumed bytes, append what arrives next, and call again. Content and header fields are borrowed from your buffer, never copied:

```rust
use base_web_multipart::{Boundary, MultipartEvent, MultipartParser};

// The body as it might arrive from a socket, in arbitrary pieces.
let chunks: [&[u8]; 3] = [b"--XyZ\r\nContent-Disposition: form-data; na", b"me=\"title\"\r\n\r\nHello, wor", b"ld\r\n--XyZ--\r\n"];

let mut parser = MultipartParser::new(&Boundary::new("XyZ")?);
let mut buffer = Vec::new();
let mut title = Vec::new();
'body: for chunk in chunks {
    buffer.extend_from_slice(chunk);
    loop {
        let parsed = parser.parse(&buffer)?;
        let consumed = parsed.consumed;
        match parsed.event {
            MultipartEvent::Part(head) => assert_eq!(head.len(), 1),
            MultipartEvent::Content(content) => title.extend_from_slice(content),
            MultipartEvent::PartEnd => {}
            MultipartEvent::End => break 'body,
            MultipartEvent::Incomplete => {
                buffer.drain(..consumed);
                break;
            }
        }
        buffer.drain(..consumed);
    }
}
// When the input ends, check that the body did too.
parser.finish()?;
assert_eq!(title, b"Hello, world");
# Ok::<(), Box<dyn std::error::Error>>(())
```

A part's header section is reported whole, so the parser may ask for more input without consuming any while it waits for the end of one. Your buffer must be able to hold `MultipartParser::minimum_buffer_size()` bytes, a little more than the header size limit; with that much, the parser always either makes progress or reports an error. Content is reported as it arrives, holding back only the few bytes that could still turn out to be the next boundary.

Each call's input must begin with the bytes the previous call did not consume. The parser relies on this to avoid searching the same bytes twice.

## Reading part headers

A part's header section is a field section, which `base-message` interprets. `PartHeaders` reads the fields this crate understands, `Content-Disposition`, `Content-Type`, and `Content-Transfer-Encoding`, and ignores the rest:

```rust
use base_message::FieldSection;
use base_web_multipart::{Boundary, PartHeaders, parts};

let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"me.png\"\r\nContent-Type: image/png\r\n\r\n\x89PNG\r\n--XyZ--";
let boundary = Boundary::new("XyZ")?;
let part = parts(body, &boundary).next().expect("one part")?;
let headers = PartHeaders::interpret(part.head())?;
let disposition = headers.content_disposition.expect("form parts have a disposition");
assert_eq!((disposition.name(), disposition.filename()), (Some("avatar"), Some("me.png")));
assert_eq!(headers.content_type.map(|content_type| content_type.media_type().essence().to_owned()).as_deref(), Some("image/png"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

The disposition borrows its name and filename from the body. A filename is what the sender chose, so check it before using it as a local path. To read more fields, declare a section of your own that flattens `PartHeaders`; see the `base-message` guides.

`parts` is a convenience for a body that is already in memory: each part's content is one slice of the body. It applies the same limits as the parser.

## Limits and errors

`MultipartLimits` bounds what a hostile body could inflate: the preamble before the first boundary, each part's header section and its number of fields, and the number of parts. The defaults are 64 KiB, 64 KiB, 64 fields, and 128 parts:

```rust
use base_web_multipart::{Boundary, MultipartErrorKind, MultipartLimits, Parts};

let limits = MultipartLimits { part_count_limit: 1, ..MultipartLimits::default() };
let body = b"--XyZ\r\n\r\none\r\n--XyZ\r\n\r\ntwo\r\n--XyZ--";
let mut parts = Parts::with_limits(body, &Boundary::new("XyZ")?, limits);
assert!(parts.next().expect("the first part").is_ok());
let error = parts.next().expect("an error").unwrap_err();
assert_eq!(error.kind(), MultipartErrorKind::PartCountLimit);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Content is not limited here. Limits on fields and uploads belong to whatever consumes the content, such as `base-web-content`'s forms.

The parser is strict where leniency would let two readers see different parts. Every line must end with CRLF; a folded header line is an error; a header name must be a valid field name; a boundary must be followed only by optional spaces and tabs and a line break, or by `--`; and a boundary after a bare CR or LF, which some readers take for a delimiter, is an error. A boundary appearing in content where RFC 2046 does not allow it is therefore an error rather than something to guess about. A body may end immediately after its closing `--`; `finish` accepts it. Errors report the offset in the body and never repeat its bytes, and a parser that has reported an error reports it again.

## Writing a body

`MultipartWriter` appends a body to your buffer. Use a generated boundary, which is 192 random bits, so that no content can contain it:

```rust
use base_web_multipart::{Boundary, ContentDisposition, MultipartWriter};

let boundary = Boundary::generate();
let content_type = boundary.form_data_media_type();
let mut writer = MultipartWriter::new(boundary);
let mut body = Vec::new();

let field = ContentDisposition::form_data("title")?.to_string();
writer.start_part([("Content-Disposition", field.as_bytes())], &mut body)?;
writer.write_content(b"Hello, world", &mut body)?;

let file = ContentDisposition::form_data("upload")?.with_parameter("filename", "notes.txt")?.to_string();
writer.start_part([("Content-Disposition", file.as_bytes()), ("Content-Type", b"text/plain")], &mut body)?;
writer.write_content(b"first chunk, ", &mut body)?;
writer.write_content(b"second chunk", &mut body)?;
writer.finish(&mut body);

assert!(content_type.to_string().starts_with("multipart/form-data; boundary="));
# Ok::<(), Box<dyn std::error::Error>>(())
```

The writer refuses whatever would change the body's structure for a reader: a field name or value with a line break or other control character, which could inject header lines, and a header line or content containing the boundary after a line break, even split across calls. A refused call writes nothing and leaves the writer as it was. The writer does not apply a reader's limits, so keep the number of parts and fields within what your readers accept.
