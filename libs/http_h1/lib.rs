// Copyright 2018-2026 the Deno authors. MIT license.

//! HTTP/1.1 protocol pieces optimized for Deno's single-threaded serve path.
//!
//! Goals:
//! - zero allocations in the steady-state request/response hot path
//! - single-threaded buffer ownership; no ref-counted byte buffers in core h1
//! - direct flat header representation
//! - minimal `unsafe`
//! - full HTTP/1.1 compliance as the implementation fills out

mod conn;
mod parse;
mod protocol;
mod read_buf;
mod write;

pub use conn::Conn;
pub use conn::Error;
pub use conn::Request;
pub use conn::Response;
pub use conn::ResponseBody;
pub use conn::ResponseHead;
pub use conn::SharedBodyChunk;
pub use conn::SharedChunkedResponseHeadWriter;
pub use conn::SharedConn;
pub use conn::SharedFixedResponseHeadWriter;
pub use conn::SharedResponseBodyWriter;
pub use conn::SharedResponseChunkWriter;
pub use conn::SharedResponseEndWriter;
pub use conn::SharedResponseWriter;
pub use conn::SharedScratch;
pub use conn::UpgradeKind;
pub use conn::UpgradeParts;
pub use parse::BodyKind;
pub use parse::Header;
pub use parse::MAX_HEADERS;
pub use parse::ParseError;
pub use parse::RequestHead;
pub use parse::Version;
pub use parse::parse_request_head;
pub use parse::parse_request_head_uninit;
pub use parse::parse_request_head_uninit_all;
pub use parse::parse_request_head_uninit_all_with_options;
pub use parse::parse_request_head_uninit_with_options;
pub use protocol::BodyStatus;
pub use protocol::CoreRequest;
pub use protocol::CoreUpgradeKind;
pub use protocol::Protocol;
pub use protocol::ProtocolError;
pub use protocol::RequestStatus;
pub use read_buf::ReadBuf;
pub use write::OutputFull;
pub use write::ResponseContentTypeFast;
pub use write::ResponseHeader;
pub use write::ResponseHeaderFast;
pub use write::append_chunk;
pub use write::append_chunk_to;
pub use write::append_chunked_end;
pub use write::append_chunked_end_to;
pub use write::content_type_response_len;
pub use write::default_text_response_len;
pub use write::status_allows_body;
pub use write::write_chunked_response_head;
pub use write::write_chunked_response_head_to;
pub use write::write_content_type_response;
pub use write::write_default_text_response;
pub use write::write_response_head;
pub use write::write_response_head_to;
