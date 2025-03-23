// Copyright 2018-2025 the Deno authors. MIT license.

mod blob;
mod compression;
mod message_port;
mod stream_resource;
mod timers;

use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::Arc;

pub use blob::BlobError;
pub use compression::CompressionError;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::ByteString;
use deno_core::ToJsBuffer;
use deno_core::U16String;
use encoding_rs::CoderResult;
use encoding_rs::Decoder;
use encoding_rs::DecoderResult;
use encoding_rs::Encoding;
pub use message_port::MessagePortError;
pub use stream_resource::StreamResourceError;

use crate::blob::op_blob_create_object_url;
use crate::blob::op_blob_create_part;
use crate::blob::op_blob_from_object_url;
use crate::blob::op_blob_read_part;
use crate::blob::op_blob_remove_part;
use crate::blob::op_blob_revoke_object_url;
use crate::blob::op_blob_slice_part;
pub use crate::blob::Blob;
pub use crate::blob::BlobPart;
pub use crate::blob::BlobStore;
pub use crate::blob::InMemoryBlobPart;
pub use crate::message_port::create_entangled_message_port;
pub use crate::message_port::deserialize_js_transferables;
use crate::message_port::op_message_port_create_entangled;
use crate::message_port::op_message_port_post_message;
use crate::message_port::op_message_port_recv_message;
use crate::message_port::op_message_port_recv_message_sync;
pub use crate::message_port::serialize_transferables;
pub use crate::message_port::JsMessageData;
pub use crate::message_port::MessagePort;
pub use crate::message_port::Transferable;
use crate::timers::op_defer;
use crate::timers::op_now;
use crate::timers::op_time_origin;
pub use crate::timers::StartTime;
pub use crate::timers::TimersPermission;

deno_core::extension!(deno_web,
  deps = [ deno_webidl, deno_console, deno_url ],
  parameters = [P: TimersPermission],
  ops = [
    op_base64_decode,
    op_base64_encode,
    op_base64_atob,
    op_base64_btoa,
    op_encoding_normalize_label,
    op_encoding_decode_single,
    op_encoding_decode_utf8,
    op_encoding_new_decoder,
    op_encoding_decode,
    op_encoding_encode_into,
    op_blob_create_part,
    op_blob_slice_part,
    op_blob_read_part,
    op_blob_remove_part,
    op_blob_create_object_url,
    op_blob_revoke_object_url,
    op_blob_from_object_url,
    op_message_port_create_entangled,
    op_message_port_post_message,
    op_message_port_recv_message,
    op_message_port_recv_message_sync,
    compression::op_compression_new,
    compression::op_compression_write,
    compression::op_compression_finish,
    op_now<P>,
    op_time_origin<P>,
    op_defer,
    stream_resource::op_readable_stream_resource_allocate,
    stream_resource::op_readable_stream_resource_allocate_sized,
    stream_resource::op_readable_stream_resource_get_sink,
    stream_resource::op_readable_stream_resource_write_error,
    stream_resource::op_readable_stream_resource_write_buf,
    stream_resource::op_readable_stream_resource_write_sync,
    stream_resource::op_readable_stream_resource_close,
    stream_resource::op_readable_stream_resource_await_close,
  ],
  esm = [
    "00_infra.js",
    "01_dom_exception.js",
    "01_mimesniff.js",
    "02_event.js",
    "02_structured_clone.js",
    "02_timers.js",
    "03_abort_signal.js",
    "04_global_interfaces.js",
    "05_base64.js",
    "06_streams.js",
    "08_text_encoding.js",
    "09_file.js",
    "10_filereader.js",
    "12_location.js",
    "13_message_port.js",
    "14_compression.js",
    "15_performance.js",
    "16_image_data.js",
  ],
  lazy_loaded_esm = [ "webtransport.js" ],
  options = {
    blob_store: Arc<BlobStore>,
    maybe_location: Option<Url>,
  },
  state = |state, options| {
    state.put(options.blob_store);
    if let Some(location) = options.maybe_location {
      state.put(Location(location));
    }
    state.put(StartTime::default());
  }
);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WebError {
  #[class("DOMExceptionInvalidCharacterError")]
  #[error("Failed to decode base64")]
  Base64Decode,
  #[class(range)]
  #[error("The encoding label provided ('{0}') is invalid.")]
  InvalidEncodingLabel(String),
  #[class(type)]
  #[error("buffer exceeds maximum length")]
  BufferTooLong,
  #[class(range)]
  #[error("Value too large to decode")]
  ValueTooLarge,
  #[class(range)]
  #[error("Provided buffer too small")]
  BufferTooSmall,
  #[class(type)]
  #[error("The encoded data is not valid")]
  DataInvalid,
  #[class(generic)]
  #[error(transparent)]
  DataError(#[from] v8::DataError),
}

#[op2]
#[serde]
fn op_base64_decode(#[string] input: String) -> Result<ToJsBuffer, WebError> {
  let mut s = input.into_bytes();
  let decoded_len = forgiving_base64_decode_inplace(&mut s)?;
  s.truncate(decoded_len);
  Ok(s.into())
}

#[op2]
#[serde]
fn op_base64_atob(#[serde] mut s: ByteString) -> Result<ByteString, WebError> {
  let decoded_len = forgiving_base64_decode_inplace(&mut s)?;
  s.truncate(decoded_len);
  Ok(s)
}

/// See <https://infra.spec.whatwg.org/#forgiving-base64>
#[inline]
fn forgiving_base64_decode_inplace(
  input: &mut [u8],
) -> Result<usize, WebError> {
  let decoded = base64_simd::forgiving_decode_inplace(input)
    .map_err(|_| WebError::Base64Decode)?;
  Ok(decoded.len())
}

#[op2]
#[string]
fn op_base64_encode(#[buffer] s: &[u8]) -> String {
  forgiving_base64_encode(s)
}

#[op2]
#[string]
fn op_base64_btoa(#[serde] s: ByteString) -> String {
  forgiving_base64_encode(s.as_ref())
}

/// See <https://infra.spec.whatwg.org/#forgiving-base64>
#[inline]
pub fn forgiving_base64_encode(s: &[u8]) -> String {
  base64_simd::STANDARD.encode_to_string(s)
}

#[op2]
#[string]
fn op_encoding_normalize_label(
  #[string] label: String,
) -> Result<String, WebError> {
  let encoding = Encoding::for_label_no_replacement(label.as_bytes())
    .ok_or(WebError::InvalidEncodingLabel(label))?;
  Ok(encoding.name().to_lowercase())
}

#[op2]
fn op_encoding_decode_utf8<'a>(
  scope: &mut v8::HandleScope<'a>,
  #[anybuffer] zero_copy: &[u8],
  ignore_bom: bool,
) -> Result<v8::Local<'a, v8::String>, WebError> {
  let buf = &zero_copy;

  let buf = if !ignore_bom
    && buf.len() >= 3
    && buf[0] == 0xef
    && buf[1] == 0xbb
    && buf[2] == 0xbf
  {
    &buf[3..]
  } else {
    buf
  };

  // If `String::new_from_utf8()` returns `None`, this means that the
  // length of the decoded string would be longer than what V8 can
  // handle. In this case we return `RangeError`.
  //
  // For more details see:
  // - https://encoding.spec.whatwg.org/#dom-textdecoder-decode
  // - https://github.com/denoland/deno/issues/6649
  // - https://github.com/v8/v8/blob/d68fb4733e39525f9ff0a9222107c02c28096e2a/include/v8.h#L3277-L3278
  match v8::String::new_from_utf8(scope, buf, v8::NewStringType::Normal) {
    Some(text) => Ok(text),
    None => Err(WebError::BufferTooLong),
  }
}

#[op2]
#[serde]
fn op_encoding_decode_single(
  #[anybuffer] data: &[u8],
  #[string] label: String,
  fatal: bool,
  ignore_bom: bool,
) -> Result<U16String, WebError> {
  let encoding = Encoding::for_label(label.as_bytes())
    .ok_or(WebError::InvalidEncodingLabel(label))?;

  let mut decoder = if ignore_bom {
    encoding.new_decoder_without_bom_handling()
  } else {
    encoding.new_decoder_with_bom_removal()
  };

  let max_buffer_length = decoder
    .max_utf16_buffer_length(data.len())
    .ok_or(WebError::ValueTooLarge)?;

  let mut output = vec![0; max_buffer_length];

  if fatal {
    let (result, _, written) =
      decoder.decode_to_utf16_without_replacement(data, &mut output, true);
    match result {
      DecoderResult::InputEmpty => {
        output.truncate(written);
        Ok(output.into())
      }
      DecoderResult::OutputFull => Err(WebError::BufferTooSmall),
      DecoderResult::Malformed(_, _) => Err(WebError::DataInvalid),
    }
  } else {
    let (result, _, written, _) =
      decoder.decode_to_utf16(data, &mut output, true);
    match result {
      CoderResult::InputEmpty => {
        output.truncate(written);
        Ok(output.into())
      }
      CoderResult::OutputFull => Err(WebError::BufferTooSmall),
    }
  }
}

#[op2]
#[cppgc]
fn op_encoding_new_decoder(
  #[string] label: &str,
  fatal: bool,
  ignore_bom: bool,
) -> Result<TextDecoderResource, WebError> {
  let encoding = Encoding::for_label(label.as_bytes())
    .ok_or_else(|| WebError::InvalidEncodingLabel(label.to_string()))?;

  let decoder = if ignore_bom {
    encoding.new_decoder_without_bom_handling()
  } else {
    encoding.new_decoder_with_bom_removal()
  };

  Ok(TextDecoderResource {
    decoder: RefCell::new(decoder),
    fatal,
  })
}

#[op2]
#[serde]
fn op_encoding_decode(
  #[anybuffer] data: &[u8],
  #[cppgc] resource: &TextDecoderResource,
  stream: bool,
) -> Result<U16String, WebError> {
  let mut decoder = resource.decoder.borrow_mut();
  let fatal = resource.fatal;

  let max_buffer_length = decoder
    .max_utf16_buffer_length(data.len())
    .ok_or(WebError::ValueTooLarge)?;

  let mut output = vec![0; max_buffer_length];

  if fatal {
    let (result, _, written) =
      decoder.decode_to_utf16_without_replacement(data, &mut output, !stream);
    match result {
      DecoderResult::InputEmpty => {
        output.truncate(written);
        Ok(output.into())
      }
      DecoderResult::OutputFull => Err(WebError::BufferTooSmall),
      DecoderResult::Malformed(_, _) => Err(WebError::DataInvalid),
    }
  } else {
    let (result, _, written, _) =
      decoder.decode_to_utf16(data, &mut output, !stream);
    match result {
      CoderResult::InputEmpty => {
        output.truncate(written);
        Ok(output.into())
      }
      CoderResult::OutputFull => Err(WebError::BufferTooSmall),
    }
  }
}

struct TextDecoderResource {
  decoder: RefCell<Decoder>,
  fatal: bool,
}

impl deno_core::GarbageCollected for TextDecoderResource {}

#[op2(fast(op_encoding_encode_into_fast))]
#[allow(deprecated)]
fn op_encoding_encode_into(
  scope: &mut v8::HandleScope,
  input: v8::Local<v8::Value>,
  #[buffer] buffer: &mut [u8],
  #[buffer] out_buf: &mut [u32],
) -> Result<(), WebError> {
  let s = v8::Local::<v8::String>::try_from(input)?;

  let mut nchars = 0;
  out_buf[1] = s.write_utf8(
    scope,
    buffer,
    Some(&mut nchars),
    v8::WriteOptions::NO_NULL_TERMINATION
      | v8::WriteOptions::REPLACE_INVALID_UTF8,
  ) as u32;
  out_buf[0] = nchars as u32;
  Ok(())
}

#[op2(fast)]
fn op_encoding_encode_into_fast(
  #[string] input: Cow<'_, str>,
  #[buffer] buffer: &mut [u8],
  #[buffer] out_buf: &mut [u32],
) {
  // Since `input` is already UTF-8, we can simply find the last UTF-8 code
  // point boundary from input that fits in `buffer`, and copy the bytes up to
  // that point.
  let boundary = if buffer.len() >= input.len() {
    input.len()
  } else {
    let mut boundary = buffer.len();

    // The maximum length of a UTF-8 code point is 4 bytes.
    for _ in 0..4 {
      if input.is_char_boundary(boundary) {
        break;
      }
      debug_assert!(boundary > 0);
      boundary -= 1;
    }

    debug_assert!(input.is_char_boundary(boundary));
    boundary
  };

  buffer[..boundary].copy_from_slice(input[..boundary].as_bytes());

  // The `read` output parameter is measured in UTF-16 code units.
  out_buf[0] = match input {
    // Borrowed Cow strings are zero-copy views into the V8 heap.
    // Thus, they are guarantee to be SeqOneByteString.
    Cow::Borrowed(v) => v[..boundary].len() as u32,
    Cow::Owned(v) => v[..boundary].encode_utf16().count() as u32,
  };
  out_buf[1] = boundary as u32;
}

pub struct Location(pub Url);
