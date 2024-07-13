// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod blob;
mod compression;
mod message_port;
mod stream_resource;
mod timers;

use deno_core::error::range_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::ByteString;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::U16String;

use encoding_rs::CoderResult;
use encoding_rs::Decoder;
use encoding_rs::DecoderResult;
use encoding_rs::Encoding;
use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

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
use crate::timers::StartTime;
pub use crate::timers::TimersPermission;

deno_core::extension!(deno_web,
  deps = [ deno_webidl, deno_console, deno_url ],
  parameters = [P: TimersPermission],
  ops = [
    op_base64_decode,
    op_base64_write,
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
  options = {
    blob_store: Arc<BlobStore>,
    maybe_location: Option<Url>,
  },
  state = |state, options| {
    state.put(options.blob_store);
    if let Some(location) = options.maybe_location {
      state.put(Location(location));
    }
    state.put(StartTime::now());
  }
);

#[op2]
#[buffer]
fn op_base64_decode(#[string] input: String) -> Result<Vec<u8>, AnyError> {
  let mut s = input.into_bytes();
  let decoded_len = forgiving_base64_decode_inplace(&mut s)?;
  s.truncate(decoded_len);
  Ok(s)
}

#[op2(fast)]
#[smi]
fn op_base64_write(
  #[string] input: String,
  #[buffer] buffer: &mut [u8],
  #[smi] start: u32,
  #[smi] max_len: u32,
) -> Result<u32, AnyError> {
  let tsb_len = buffer.len() as u32;

  if start > tsb_len {
    return Err(type_error("Offset is out of bounds"));
  }

  let max_len = std::cmp::min(max_len, tsb_len - start) as usize;
  let start = start as usize;

  if max_len == 0 {
    return Ok(0);
  }

  let mut s = input.into_bytes();
  let decoded_len = forgiving_base64_decode_inplace(&mut s)?;

  buffer[start..start + max_len].copy_from_slice(&s[..max_len]);

  Ok(decoded_len as u32)
}

#[op2]
#[serde]
fn op_base64_atob(#[serde] mut s: ByteString) -> Result<ByteString, AnyError> {
  let decoded_len = forgiving_base64_decode_inplace(&mut s)?;
  s.truncate(decoded_len);
  Ok(s)
}

/// See <https://infra.spec.whatwg.org/#forgiving-base64>
#[inline]
fn forgiving_base64_decode_inplace(
  input: &mut [u8],
) -> Result<usize, AnyError> {
  let error =
    || DomExceptionInvalidCharacterError::new("Failed to decode base64");
  let decoded =
    base64_simd::forgiving_decode_inplace(input).map_err(|_| error())?;
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
fn forgiving_base64_encode(s: &[u8]) -> String {
  base64_simd::STANDARD.encode_to_string(s)
}

#[op2]
#[string]
fn op_encoding_normalize_label(
  #[string] label: String,
) -> Result<String, AnyError> {
  let encoding = Encoding::for_label_no_replacement(label.as_bytes())
    .ok_or_else(|| {
      range_error(format!(
        "The encoding label provided ('{label}') is invalid."
      ))
    })?;
  Ok(encoding.name().to_lowercase())
}

#[op2]
fn op_encoding_decode_utf8<'a>(
  scope: &mut v8::HandleScope<'a>,
  #[anybuffer] zero_copy: &[u8],
  ignore_bom: bool,
) -> Result<v8::Local<'a, v8::String>, AnyError> {
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
    None => Err(type_error("buffer exceeds maximum length")),
  }
}

#[op2]
#[serde]
fn op_encoding_decode_single(
  #[anybuffer] data: &[u8],
  #[string] label: String,
  fatal: bool,
  ignore_bom: bool,
) -> Result<U16String, AnyError> {
  let encoding = Encoding::for_label(label.as_bytes()).ok_or_else(|| {
    range_error(format!(
      "The encoding label provided ('{label}') is invalid."
    ))
  })?;

  let mut decoder = if ignore_bom {
    encoding.new_decoder_without_bom_handling()
  } else {
    encoding.new_decoder_with_bom_removal()
  };

  let max_buffer_length = decoder
    .max_utf16_buffer_length(data.len())
    .ok_or_else(|| range_error("Value too large to decode."))?;

  let mut output = vec![0; max_buffer_length];

  if fatal {
    let (result, _, written) =
      decoder.decode_to_utf16_without_replacement(data, &mut output, true);
    match result {
      DecoderResult::InputEmpty => {
        output.truncate(written);
        Ok(output.into())
      }
      DecoderResult::OutputFull => {
        Err(range_error("Provided buffer too small."))
      }
      DecoderResult::Malformed(_, _) => {
        Err(type_error("The encoded data is not valid."))
      }
    }
  } else {
    let (result, _, written, _) =
      decoder.decode_to_utf16(data, &mut output, true);
    match result {
      CoderResult::InputEmpty => {
        output.truncate(written);
        Ok(output.into())
      }
      CoderResult::OutputFull => Err(range_error("Provided buffer too small.")),
    }
  }
}

#[op2(fast)]
#[smi]
fn op_encoding_new_decoder(
  state: &mut OpState,
  #[string] label: &str,
  fatal: bool,
  ignore_bom: bool,
) -> Result<ResourceId, AnyError> {
  let encoding = Encoding::for_label(label.as_bytes()).ok_or_else(|| {
    range_error(format!(
      "The encoding label provided ('{label}') is invalid."
    ))
  })?;

  let decoder = if ignore_bom {
    encoding.new_decoder_without_bom_handling()
  } else {
    encoding.new_decoder_with_bom_removal()
  };

  let rid = state.resource_table.add(TextDecoderResource {
    decoder: RefCell::new(decoder),
    fatal,
  });

  Ok(rid)
}

#[op2]
#[serde]
fn op_encoding_decode(
  state: &mut OpState,
  #[anybuffer] data: &[u8],
  #[smi] rid: ResourceId,
  stream: bool,
) -> Result<U16String, AnyError> {
  let resource = state.resource_table.get::<TextDecoderResource>(rid)?;

  let mut decoder = resource.decoder.borrow_mut();
  let fatal = resource.fatal;

  let max_buffer_length = decoder
    .max_utf16_buffer_length(data.len())
    .ok_or_else(|| range_error("Value too large to decode."))?;

  let mut output = vec![0; max_buffer_length];

  if fatal {
    let (result, _, written) =
      decoder.decode_to_utf16_without_replacement(data, &mut output, !stream);
    match result {
      DecoderResult::InputEmpty => {
        output.truncate(written);
        Ok(output.into())
      }
      DecoderResult::OutputFull => {
        Err(range_error("Provided buffer too small."))
      }
      DecoderResult::Malformed(_, _) => {
        Err(type_error("The encoded data is not valid."))
      }
    }
  } else {
    let (result, _, written, _) =
      decoder.decode_to_utf16(data, &mut output, !stream);
    match result {
      CoderResult::InputEmpty => {
        output.truncate(written);
        Ok(output.into())
      }
      CoderResult::OutputFull => Err(range_error("Provided buffer too small.")),
    }
  }
}

struct TextDecoderResource {
  decoder: RefCell<Decoder>,
  fatal: bool,
}

impl Resource for TextDecoderResource {
  fn name(&self) -> Cow<str> {
    "textDecoder".into()
  }
}

#[op2(fast(op_encoding_encode_into_fast))]
fn op_encoding_encode_into(
  scope: &mut v8::HandleScope,
  input: v8::Local<v8::Value>,
  #[buffer] buffer: &mut [u8],
  #[buffer] out_buf: &mut [u32],
) -> Result<(), AnyError> {
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

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_web.d.ts")
}

#[derive(Debug)]
pub struct DomExceptionQuotaExceededError {
  pub msg: String,
}

impl DomExceptionQuotaExceededError {
  pub fn new(msg: &str) -> Self {
    DomExceptionQuotaExceededError {
      msg: msg.to_string(),
    }
  }
}

#[derive(Debug)]
pub struct DomExceptionInvalidCharacterError {
  pub msg: String,
}

impl DomExceptionInvalidCharacterError {
  pub fn new(msg: &str) -> Self {
    DomExceptionInvalidCharacterError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DomExceptionQuotaExceededError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}
impl fmt::Display for DomExceptionInvalidCharacterError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DomExceptionQuotaExceededError {}

impl std::error::Error for DomExceptionInvalidCharacterError {}

pub fn get_error_class_name(e: &AnyError) -> Option<&'static str> {
  e.downcast_ref::<DomExceptionQuotaExceededError>()
    .map(|_| "DOMExceptionQuotaExceededError")
    .or_else(|| {
      e.downcast_ref::<DomExceptionInvalidCharacterError>()
        .map(|_| "DOMExceptionInvalidCharacterError")
    })
}
pub struct Location(pub Url);
