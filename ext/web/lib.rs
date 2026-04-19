// Copyright 2018-2026 the Deno authors. MIT license.

mod blob;

mod broadcast_channel;
mod compression;
mod console;
mod message_port;
mod stream_resource;
mod timers;
mod url;
mod urlpattern;

use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::Arc;

pub use blob::BlobError;
pub use compression::CompressionError;
use deno_core::U16String;
use deno_core::convert::ByteString;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::v8;
use encoding_rs::CoderResult;
use encoding_rs::Decoder;
use encoding_rs::DecoderResult;
use encoding_rs::Encoding;
pub use message_port::MessagePortError;
pub use stream_resource::StreamResourceError;

pub use crate::blob::Blob;
pub use crate::blob::BlobPart;
pub use crate::blob::BlobStore;
pub use crate::blob::InMemoryBlobPart;
use crate::blob::op_blob_create_object_url;
use crate::blob::op_blob_create_part;
use crate::blob::op_blob_from_object_url;
use crate::blob::op_blob_read_part;
use crate::blob::op_blob_remove_part;
use crate::blob::op_blob_revoke_object_url;
use crate::blob::op_blob_slice_part;
pub use crate::broadcast_channel::InMemoryBroadcastChannel;
pub use crate::message_port::JsMessageData;
pub use crate::message_port::MessagePort;
pub use crate::message_port::Transferable;
pub use crate::message_port::create_entangled_message_port;
pub use crate::message_port::deserialize_js_transferables;
use crate::message_port::op_message_port_create_entangled;
use crate::message_port::op_message_port_post_message;
use crate::message_port::op_message_port_recv_message;
use crate::message_port::op_message_port_recv_message_sync;
pub use crate::message_port::serialize_transferables;
pub use crate::timers::StartTime;
use crate::timers::op_defer;
use crate::timers::op_now;
use crate::timers::op_time_origin;

deno_core::extension!(deno_web,
  deps = [ deno_webidl ],
  ops = [
    op_base64_decode,
    op_base64_decode_into,
    op_base64_encode,
    op_base64_encode_from_buffer,
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
    op_now,
    op_time_origin,
    op_defer,
    stream_resource::op_readable_stream_resource_allocate,
    stream_resource::op_readable_stream_resource_allocate_sized,
    stream_resource::op_readable_stream_resource_get_sink,
    stream_resource::op_readable_stream_resource_write_error,
    stream_resource::op_readable_stream_resource_write_buf,
    stream_resource::op_readable_stream_resource_write_sync,
    stream_resource::op_readable_stream_resource_close,
    stream_resource::op_readable_stream_resource_await_close,
    url::op_url_reparse,
    url::op_url_parse,
    url::op_url_get_serialization,
    url::op_url_parse_with_base,
    url::op_url_parse_search_params,
    url::op_url_stringify_search_params,
    urlpattern::op_urlpattern_parse,
    urlpattern::op_urlpattern_process_match_input,
    console::op_preview_entries,
    broadcast_channel::op_broadcast_subscribe,
    broadcast_channel::op_broadcast_unsubscribe,
    broadcast_channel::op_broadcast_send,
    broadcast_channel::op_broadcast_recv,
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
    "00_url.js",
    "01_urlpattern.js",
    "01_console.js",
    "01_broadcast_channel.js"
  ],
  lazy_loaded_esm = [ "webtransport.js" ],
  options = {
    blob_store: Arc<BlobStore>,
    maybe_location: Option<Url>,
    bc: InMemoryBroadcastChannel,
  },
  state = |state, options| {
    state.put(options.blob_store);
    if let Some(location) = options.maybe_location {
      state.put(Location(location));
    }
    state.put(StartTime::default());
    state.put(options.bc);
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

/// Forgiving base64 decode using simdutf. Decodes into a new Vec.
/// Handles whitespace stripping and missing padding (Loose mode).
#[inline]
fn simdutf_base64_decode_to_vec(input: &[u8]) -> Result<Vec<u8>, WebError> {
  use v8::simdutf;
  let max_len = simdutf::maximal_binary_length_from_base64(input);
  let mut output = Vec::with_capacity(max_len);
  // Safety: output has max_len bytes of capacity which is >= decoded size.
  // ffi_base64_to_binary writes into the buffer without reading uninitialized data.
  let result = unsafe {
    ffi_base64_to_binary(
      input.as_ptr(),
      input.len(),
      output.as_mut_ptr(),
      simdutf::Base64Options::Default as u64,
      simdutf::LastChunkHandling::Loose as u64,
    )
  };
  // error == 0 means success (simdutf error_code::SUCCESS)
  if result.error != 0 {
    return Err(WebError::Base64Decode);
  }
  // Safety: base64_to_binary wrote result.count bytes.
  unsafe { output.set_len(result.count) };
  Ok(output)
}

/// Forgiving base64 decode into an existing buffer using simdutf.
/// Returns the number of bytes written.
#[inline]
fn simdutf_base64_decode_into(
  input: &[u8],
  output: &mut [u8],
) -> Result<usize, WebError> {
  use v8::simdutf;
  // Safety: caller provides output buffer with sufficient capacity.
  let result = unsafe {
    simdutf::base64_to_binary(
      input,
      output,
      simdutf::Base64Options::Default,
      simdutf::LastChunkHandling::Loose,
    )
  };
  if !result.is_ok() {
    return Err(WebError::Base64Decode);
  }
  Ok(result.count)
}

/// Strict base64 decode directly into target buffer.
/// Returns None if the input is not valid strict padded base64.
#[inline]
fn simdutf_base64_decode_strict(
  input: &[u8],
  output: &mut [u8],
) -> Option<usize> {
  use v8::simdutf;
  // Safety: caller provides output buffer with sufficient capacity.
  let result = unsafe {
    simdutf::base64_to_binary(
      input,
      output,
      simdutf::Base64Options::Default,
      simdutf::LastChunkHandling::Strict,
    )
  };
  if result.is_ok() {
    Some(result.count)
  } else {
    None
  }
}

// Re-declare simdutf FFI functions to allow passing raw pointers
// without constructing &mut [u8] from uninitialized memory (which is UB).
#[repr(C)]
struct SimdutfFfiResult {
  error: i32,
  count: usize,
}

unsafe extern "C" {
  #[link_name = "simdutf__binary_to_base64"]
  fn ffi_binary_to_base64(
    input: *const u8,
    length: usize,
    output: *mut u8,
    options: u64,
  ) -> usize;

  #[link_name = "simdutf__base64_to_binary"]
  fn ffi_base64_to_binary(
    input: *const u8,
    length: usize,
    output: *mut u8,
    options: u64,
    last_chunk_options: u64,
  ) -> SimdutfFfiResult;
}

/// Encode binary to base64 using simdutf. Returns encoded length.
///
/// # Safety
/// `output` must point to at least `base64_length_from_binary(input.len())`
/// writable bytes. The bytes do not need to be initialized.
#[inline]
unsafe fn simdutf_base64_encode(
  input: &[u8],
  output: *mut u8,
  output_len: usize,
) -> usize {
  debug_assert!(
    output_len
      >= v8::simdutf::base64_length_from_binary(
        input.len(),
        v8::simdutf::Base64Options::Default
      )
  );
  // Safety: caller guarantees output has sufficient capacity.
  unsafe {
    ffi_binary_to_base64(
      input.as_ptr(),
      input.len(),
      output,
      v8::simdutf::Base64Options::Default as u64,
    )
  }
}

#[op2]
fn op_base64_decode(
  #[string(onebyte)] input: Cow<[u8]>,
) -> Result<Uint8Array, WebError> {
  let v = simdutf_base64_decode_to_vec(&input)?;
  Ok(v.into())
}

/// Decode base64 directly into a target buffer at the given offset.
/// Returns the number of bytes written.
///
/// Fast path: tries strict decode directly into target (zero intermediate
/// copies). This works for properly-padded base64 without whitespace.
/// Slow path: uses forgiving decode for inputs with whitespace or missing
/// padding.
#[op2(fast)]
fn op_base64_decode_into(
  #[string(onebyte)] input: Cow<[u8]>,
  #[buffer] target: &mut [u8],
  #[smi] offset: u32,
) -> Result<u32, WebError> {
  let offset = offset as usize;
  let target = &mut target[offset..];

  // Fast path: try strict decode directly into target.
  // Works for clean padded base64 (the common case).
  let max_len = v8::simdutf::maximal_binary_length_from_base64(&input);
  if target.len() >= max_len
    && let Some(len) = simdutf_base64_decode_strict(&input, target)
  {
    return Ok(len as u32);
  }

  // Slow path: forgiving decode for whitespace/missing padding.
  const STACK_BUF_SIZE: usize = 8192;
  if max_len <= STACK_BUF_SIZE {
    let mut buf = std::mem::MaybeUninit::<[u8; STACK_BUF_SIZE]>::uninit();
    // Safety: simdutf writes into buf without reading uninitialized data.
    let decoded_len = simdutf_base64_decode_into(&input, unsafe {
      std::slice::from_raw_parts_mut(
        buf.as_mut_ptr() as *mut u8,
        STACK_BUF_SIZE,
      )
    })?;
    let bytes_to_write = decoded_len.min(target.len());
    // Safety: decoded_len bytes were written by simdutf.
    target[..bytes_to_write].copy_from_slice(unsafe {
      std::slice::from_raw_parts(buf.as_ptr() as *const u8, bytes_to_write)
    });
    Ok(bytes_to_write as u32)
  } else {
    let decoded = simdutf_base64_decode_to_vec(&input)?;
    let bytes_to_write = decoded.len().min(target.len());
    target[..bytes_to_write].copy_from_slice(&decoded[..bytes_to_write]);
    Ok(bytes_to_write as u32)
  }
}

#[op2]
fn op_base64_atob(#[scoped] mut s: ByteString) -> Result<ByteString, WebError> {
  // Decode into a temporary buffer — simdutf requires non-overlapping buffers.
  let max_len = v8::simdutf::maximal_binary_length_from_base64(&s);
  const STACK_BUF_SIZE: usize = 8192;
  if max_len <= STACK_BUF_SIZE {
    let mut buf = std::mem::MaybeUninit::<[u8; STACK_BUF_SIZE]>::uninit();
    // Safety: simdutf writes into buf without reading uninitialized data.
    let decoded_len = simdutf_base64_decode_into(&s, unsafe {
      std::slice::from_raw_parts_mut(
        buf.as_mut_ptr() as *mut u8,
        STACK_BUF_SIZE,
      )
    })?;
    // Safety: decoded_len bytes were written by simdutf.
    s[..decoded_len].copy_from_slice(unsafe {
      std::slice::from_raw_parts(buf.as_ptr() as *const u8, decoded_len)
    });
    s.truncate(decoded_len);
    Ok(s)
  } else {
    let decoded = simdutf_base64_decode_to_vec(&s)?;
    let decoded_len = decoded.len();
    s[..decoded_len].copy_from_slice(&decoded[..decoded_len]);
    s.truncate(decoded_len);
    Ok(s)
  }
}

#[op2]
#[string]
fn op_base64_encode(#[buffer] s: &[u8]) -> String {
  forgiving_base64_encode(s)
}

/// Encode a sub-range of a buffer to base64, avoiding a JS-side slice copy.
#[op2]
fn op_base64_encode_from_buffer<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[buffer] s: &[u8],
  #[smi] offset: u32,
  #[smi] length: u32,
) -> Result<v8::Local<'a, v8::String>, WebError> {
  let offset = offset as usize;
  let length = length as usize;
  let end = (offset + length).min(s.len());
  base64_encode_to_v8_string(scope, &s[offset..end])
}

/// Encode bytes to base64 and create a V8 one-byte string directly.
/// Stack-allocates for inputs producing ≤8KB base64.
/// Uses v8::String::new_external_onebyte for large outputs to avoid copying.
#[inline]
fn base64_encode_to_v8_string<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  src: &[u8],
) -> Result<v8::Local<'a, v8::String>, WebError> {
  let b64_len = v8::simdutf::base64_length_from_binary(
    src.len(),
    v8::simdutf::Base64Options::Default,
  );

  const STACK_BUF_SIZE: usize = 8192;
  if b64_len <= STACK_BUF_SIZE {
    let mut buf = std::mem::MaybeUninit::<[u8; STACK_BUF_SIZE]>::uninit();
    // Safety: buf has STACK_BUF_SIZE >= b64_len bytes.
    // simdutf writes `written` bytes without reading uninitialized data.
    let written = unsafe {
      simdutf_base64_encode(src, buf.as_mut_ptr() as *mut u8, b64_len)
    };
    v8::String::new_from_one_byte(
      scope,
      // Safety: written <= b64_len <= STACK_BUF_SIZE, all initialized.
      unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, written) },
      v8::NewStringType::Normal,
    )
    .ok_or(WebError::BufferTooLong)
  } else {
    // Encode into a boxed slice and hand ownership to V8 via external string.
    // This avoids a copy — V8 will free the buffer when the string is GC'd.
    let mut buf = Vec::with_capacity(b64_len);
    // Safety: buf has b64_len bytes of capacity.
    // binary_to_base64 writes exactly b64_len bytes without reading.
    let written =
      unsafe { simdutf_base64_encode(src, buf.as_mut_ptr(), b64_len) };
    // Safety: written bytes are initialized by binary_to_base64.
    unsafe { buf.set_len(written) };
    let buf = buf.into_boxed_slice();
    debug_assert_eq!(written, b64_len);
    v8::String::new_external_onebyte(scope, buf).ok_or(WebError::BufferTooLong)
  }
}

#[op2]
#[string]
fn op_base64_btoa(#[scoped] s: ByteString) -> String {
  forgiving_base64_encode(s.as_ref())
}

/// See <https://infra.spec.whatwg.org/#forgiving-base64>
#[inline]
pub fn forgiving_base64_encode(s: &[u8]) -> String {
  let b64_len = v8::simdutf::base64_length_from_binary(
    s.len(),
    v8::simdutf::Base64Options::Default,
  );
  let mut buf = Vec::with_capacity(b64_len);
  // Safety: buf has b64_len bytes of capacity.
  // binary_to_base64 writes up to b64_len bytes, all valid ASCII.
  unsafe {
    let written = simdutf_base64_encode(s, buf.as_mut_ptr(), b64_len);
    buf.set_len(written);
    String::from_utf8_unchecked(buf)
  }
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
  scope: &mut v8::PinScope<'a, '_>,
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

// SAFETY: we're sure `TextDecoderResource` can be GCed
unsafe impl deno_core::GarbageCollected for TextDecoderResource {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TextDecoderResource"
  }
}

#[op2(fast(op_encoding_encode_into_fast))]
fn op_encoding_encode_into(
  scope: &mut v8::PinScope<'_, '_>,
  input: v8::Local<v8::Value>,
  #[buffer] buffer: &mut [u8],
  #[buffer] out_buf: &mut [u32],
) -> Result<(), WebError> {
  let s = v8::Local::<v8::String>::try_from(input)?;

  let mut nchars = 0;
  let len = s.write_utf8_v2(
    scope,
    buffer,
    v8::WriteFlags::kReplaceInvalidUtf8,
    Some(&mut nchars),
  );
  out_buf[1] = len as u32;
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
