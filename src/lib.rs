// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! MIME multipart bodies, as RFC 2046 defines them and RFC 7578 uses them for form data, parsed and written without I/O.
//!
//! [`MultipartParser`] is fed bytes as they arrive and reports each part's header section and content as borrowed slices, never copying them, with [`MultipartLimits`] on the structure a hostile body could inflate. A part's header section is interpreted with `base-message`, through [`PartHeaders`] or a section of your own, and [`ContentDisposition`] reads form field names and filenames. [`MultipartWriter`] writes a body, refusing header lines and content that would change its structure for a reader, with a [`Boundary`] generated from 192 random bits.
//!
//! ```
//! use base_web_multipart::{Boundary, ContentDisposition, MultipartWriter, PartHeaders, parts};
//! use base_message::FieldSection;
//!
//! let boundary = Boundary::generate();
//! let mut writer = MultipartWriter::new(boundary.clone());
//! let mut body = Vec::new();
//! let disposition = ContentDisposition::form_data("greeting")?.to_string();
//! writer.start_part([("Content-Disposition", disposition.as_bytes())], &mut body)?;
//! writer.write_content(b"hello", &mut body)?;
//! writer.finish(&mut body);
//!
//! for part in parts(&body, &boundary) {
//!     let part = part?;
//!     let headers = PartHeaders::interpret(part.head())?;
//!     let disposition = headers.content_disposition.expect("each form part has a disposition");
//!     assert_eq!((disposition.name(), part.content()), (Some("greeting"), &b"hello"[..]));
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! The crate frames parts only. Forms, with their limits on fields and uploads, belong to `base-web-content`, and HTTP transport to `base-http`. The [Getting Started](https://github.com/base-rs/base-web-multipart/blob/main/context/getting-started.md) guide covers parsing a streamed body and writing one.

mod boundary;
mod content_disposition;
mod content_transfer_encoding;
mod content_type;
mod multipart_error;
mod multipart_event;
mod multipart_limits;
mod multipart_parser;
mod multipart_writer;
mod part_head;
mod part_headers;
mod parts;
mod syntax;

pub use boundary::{Boundary, BoundaryError};
pub use content_disposition::ContentDisposition;
pub use content_transfer_encoding::ContentTransferEncoding;
pub use content_type::ContentType;
pub use multipart_error::{MultipartError, MultipartErrorKind};
pub use multipart_event::{MultipartEvent, Parsed};
pub use multipart_limits::MultipartLimits;
pub use multipart_parser::MultipartParser;
pub use multipart_writer::{MultipartWriteError, MultipartWriter};
pub use part_head::{PartFields, PartHead};
pub use part_headers::PartHeaders;
pub use parts::{Part, Parts, parts};

#[cfg(doctest)]
#[doc = include_str!("../readme.md")]
struct Readme;

#[cfg(doctest)]
#[doc = include_str!("../context/getting-started.md")]
struct GettingStartedGuide;
