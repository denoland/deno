// Copyright 2018-2026 the Deno authors. MIT license.

mod blob;

mod broadcast_channel;
mod compression;
mod console;
mod css_stylesheet;
mod css_value;
mod f64;
mod geometry;
mod image_data;
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
pub use css_stylesheet::create_css_style_sheet;
#[allow(deprecated, reason = "uses a deprecated serde_v8 magic type; kept until call sites migrate")]
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
pub use crate::blob::BlobStoreTrait;
pub use crate::blob::InMemoryBlobPart;
use crate::blob::op_blob_clone_part;
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
pub use crate::message_port::RecvMessageData;
pub use crate::message_port::Transferable;
pub use crate::message_port::create_entangled_message_port;
pub use crate::message_port::deserialize_js_transferables;
use crate::message_port::op_message_port_create_entangled;
use crate::message_port::op_message_port_post_message;
use crate::message_port::op_message_port_post_message_raw;
use crate::message_port::op_message_port_recv_message;
use crate::message_port::op_message_port_recv_message_sync;
pub use crate::message_port::serialize_transferables;
pub use crate::timers::StartTime;
use crate::timers::op_defer;
use crate::timers::op_now;
use crate::timers::op_time_origin;
pub mod locks;

deno_core::extension!(deno_web,
  deps = [ deno_webidl ],
  ops = [
    op_base64_decode,
    op_base64_decode_into,
    op_base64_encode_from_buffer,
    op_base64_atob,
    op_base64_btoa,
    op_base64url_decode,
    op_base64url_decode_into,
    op_base64url_encode_from_buffer,
    op_encoding_normalize_label,
    op_encoding_decode_single,
    op_encoding_decode_utf8,
    op_encoding_decode_utf8_ascii_only,
    op_encoding_new_decoder,
    op_encoding_decode,
    op_encoding_encode_into,
    op_encoding_encode_into_fallback,
    op_blob_create_part,
    op_blob_slice_part,
    op_blob_read_part,
    op_blob_remove_part,
    op_blob_clone_part,
    op_blob_create_object_url,
    op_blob_revoke_object_url,
    op_blob_from_object_url,
    op_message_port_create_entangled,
    op_message_port_post_message,
    op_message_port_post_message_raw,
    op_message_port_recv_message,
    op_message_port_recv_message_sync,
    compression::op_compression_new,
    compression::op_compression_write,
    compression::op_compression_finish,
    op_now,
    op_time_origin,
    op_defer,
    geometry::op_geometry_get_enable_css_parser_features,
    geometry::op_geometry_matrix_set_matrix_value,
    geometry::op_geometry_matrix_to_string,
    stream_resource::op_readable_stream_resource_allocate,
    stream_resource::op_readable_stream_resource_allocate_sized,
    stream_resource::op_readable_stream_resource_get_sink,
    stream_resource::op_readable_stream_resource_write_error,
    stream_resource::op_readable_stream_resource_write_buf,
    stream_resource::op_readable_stream_resource_write_sync,
    stream_resource::op_readable_stream_resource_close,
    stream_resource::op_readable_stream_resource_await_close,
    locks::op_lock_manager_request,
    locks::op_lock_manager_await_lock,
    locks::op_lock_manager_await_steal,
    locks::op_lock_manager_is_stolen,
    locks::op_lock_manager_cancel,
    locks::op_lock_manager_release,
    locks::op_lock_manager_query,
    url::op_url_reparse,
    url::op_url_parse,
    url::op_url_get_serialization,
    url::op_url_parse_with_base,
    url::op_url_parse_search_params,
    url::op_url_stringify_search_params,
    urlpattern::op_urlpattern_parse,
    urlpattern::op_urlpattern_process_match_input,
    console::op_preview_entries,
    console::op_console_inspect,
    console::op_console_inspect_args,
    console::op_console_format_value,
    console::op_console_quote_string,
    console::op_console_parse_css,
    console::op_console_parse_css_color,
    console::op_console_css_to_ansi,
    console::op_console_get_string_width,
    console::op_console_strip_vt,
    broadcast_channel::op_broadcast_subscribe,
    broadcast_channel::op_broadcast_unsubscribe,
    broadcast_channel::op_broadcast_serialize,
    broadcast_channel::op_broadcast_deserialize,
    broadcast_channel::op_broadcast_free,
    broadcast_channel::op_broadcast_send,
    broadcast_channel::op_broadcast_recv,
  ],
  objects = [
    css_stylesheet::CSSRule,
    css_stylesheet::CSSStyleSheet,
    geometry::DOMPointReadOnly,
    geometry::DOMPoint,
    geometry::DOMRectReadOnly,
    geometry::DOMRect,
    geometry::DOMQuad,
    geometry::DOMMatrixReadOnly,
    geometry::DOMMatrix,
    image_data::ImageData,
    console::Console,
  ],
  lazy_loaded_esm = [
    "locks.js",
    "webtransport.js",
  ],
  lazy_loaded_js = [
    "00_infra.js",
    "00_url.js",
    "01_broadcast_channel.js",
    "01_console.js",
    "01_dom_exception.js",
    "01_mimesniff.js",
    "01_urlpattern.js",
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
    "17_geometry.js",
    "18_css_stylesheet.js",
  ],
  options = {
    blob_store: Arc<dyn BlobStoreTrait>,
    maybe_location: Option<Url>,
    enable_css_parser_features: bool,
    bc: InMemoryBroadcastChannel,
  },
  state = |state, options| {
    state.put(options.blob_store);
    if let Some(location) = options.maybe_location {
      state.put(Location(location));
    }
    state.put(StartTime::default());
    state.put(geometry::State::new(options.enable_css_parser_features));
    state.put(options.bc);
    state.put(broadcast_channel::BroadcastSabStash::default());
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

/// Base64 decode using simdutf, into a new Vec. `options` selects the
/// standard or URL-safe alphabet. Loose last-chunk handling: accepts padded
/// and unpadded input and strips ASCII whitespace.
#[inline]
fn simdutf_base64_decode_to_vec(
  input: &[u8],
  options: v8::simdutf::Base64Options,
) -> Result<Vec<u8>, WebError> {
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
      options as u64,
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

/// Base64 decode into an existing buffer using simdutf.
/// Returns the number of bytes written, or None on invalid input.
#[inline]
fn simdutf_base64_decode_into(
  input: &[u8],
  output: &mut [u8],
  options: v8::simdutf::Base64Options,
  last_chunk: v8::simdutf::LastChunkHandling,
) -> Option<usize> {
  use v8::simdutf;
  // simdutf may write up to the maximal decoded length before detecting an
  // error, so an undersized output is memory-unsafe, not just wrong.
  assert!(output.len() >= simdutf::maximal_binary_length_from_base64(input));
  // Safety: output capacity checked above.
  let result =
    unsafe { simdutf::base64_to_binary(input, output, options, last_chunk) };
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
/// Output is padded for the standard alphabet and unpadded for Url.
///
/// # Safety
/// `output` must point to at least
/// `base64_length_from_binary(input.len(), options)` writable bytes. The
/// bytes do not need to be initialized.
#[inline]
unsafe fn simdutf_base64_encode(
  input: &[u8],
  output: *mut u8,
  output_len: usize,
  options: v8::simdutf::Base64Options,
) -> usize {
  debug_assert!(
    output_len >= v8::simdutf::base64_length_from_binary(input.len(), options)
  );
  // Safety: caller guarantees output has sufficient capacity.
  unsafe {
    ffi_binary_to_base64(input.as_ptr(), input.len(), output, options as u64)
  }
}

#[op2]
fn op_base64_decode(
  #[string(onebyte)] input: Cow<[u8]>,
) -> Result<Uint8Array, WebError> {
  let v =
    simdutf_base64_decode_to_vec(&input, v8::simdutf::Base64Options::Default)?;
  Ok(v.into())
}

/// Decode base64 into `target` at `offset`, truncating when the remaining
/// target is smaller than the decoded output. Returns the number of bytes
/// written, or the -1 invalid-input sentinel (see base64_decode_into_slice).
///
/// Fast path: strict decode straight into target — clean padded input is the
/// common case for the standard alphabet, unlike base64url.
#[op2(fast)]
fn op_base64_decode_into(
  #[string(onebyte)] input: Cow<[u8]>,
  #[buffer] target: &mut [u8],
  #[smi] offset: u32,
) -> Result<i32, WebError> {
  let offset = offset as usize;
  let target = target.get_mut(offset..).ok_or(WebError::BufferTooSmall)?;

  let max_len = v8::simdutf::maximal_binary_length_from_base64(&input);
  if target.len() >= max_len
    && let Some(len) = simdutf_base64_decode_into(
      &input,
      target,
      v8::simdutf::Base64Options::Default,
      v8::simdutf::LastChunkHandling::Strict,
    )
  {
    return Ok(len as i32);
  }

  Ok(base64_decode_into_slice(
    &input,
    target,
    v8::simdutf::Base64Options::Default,
  ))
}

#[op2]
fn op_base64_atob(#[scoped] mut s: ByteString) -> Result<ByteString, WebError> {
  // Decode into a temporary buffer — simdutf requires non-overlapping buffers.
  let max_len = v8::simdutf::maximal_binary_length_from_base64(&s);
  const STACK_BUF_SIZE: usize = 8192;
  if max_len <= STACK_BUF_SIZE {
    let mut buf = std::mem::MaybeUninit::<[u8; STACK_BUF_SIZE]>::uninit();
    // Safety: simdutf writes into buf without reading uninitialized data.
    let decoded_len = simdutf_base64_decode_into(
      &s,
      unsafe {
        std::slice::from_raw_parts_mut(
          buf.as_mut_ptr() as *mut u8,
          STACK_BUF_SIZE,
        )
      },
      v8::simdutf::Base64Options::Default,
      v8::simdutf::LastChunkHandling::Loose,
    )
    .ok_or(WebError::Base64Decode)?;
    // Safety: decoded_len bytes were written by simdutf.
    s[..decoded_len].copy_from_slice(unsafe {
      std::slice::from_raw_parts(buf.as_ptr() as *const u8, decoded_len)
    });
    s.truncate(decoded_len);
    Ok(s)
  } else {
    // Return the freshly decoded bytes directly rather than copying them back
    // into the (larger) input string's buffer and truncating -- this saves a
    // full-size memcpy of the output on every large `atob` call.
    Ok(
      simdutf_base64_decode_to_vec(&s, v8::simdutf::Base64Options::Default)?
        .into(),
    )
  }
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
  let end = offset.checked_add(length).ok_or(WebError::BufferTooSmall)?;
  let s = s.get(offset..end).ok_or(WebError::BufferTooSmall)?;
  base64_encode_to_v8_string(scope, s, v8::simdutf::Base64Options::Default)
}

/// Encode bytes to base64 and create a V8 one-byte string directly.
/// Stack-allocates for outputs <= 8KB; hands ownership to V8 via an external
/// string for large outputs to avoid copying.
#[inline]
fn base64_encode_to_v8_string<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  src: &[u8],
  options: v8::simdutf::Base64Options,
) -> Result<v8::Local<'a, v8::String>, WebError> {
  let b64_len = v8::simdutf::base64_length_from_binary(src.len(), options);

  const STACK_BUF_SIZE: usize = 8192;
  if b64_len <= STACK_BUF_SIZE {
    let mut buf = std::mem::MaybeUninit::<[u8; STACK_BUF_SIZE]>::uninit();
    // Safety: buf has STACK_BUF_SIZE >= b64_len bytes.
    // simdutf writes `written` bytes without reading uninitialized data.
    let written = unsafe {
      simdutf_base64_encode(src, buf.as_mut_ptr() as *mut u8, b64_len, options)
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
      unsafe { simdutf_base64_encode(src, buf.as_mut_ptr(), b64_len, options) };
    // A shorter write would make into_boxed_slice reallocate and copy.
    debug_assert_eq!(written, b64_len);
    // Safety: written bytes are initialized by binary_to_base64.
    unsafe { buf.set_len(written) };
    let buf = buf.into_boxed_slice();
    v8::String::new_external_onebyte(scope, buf).ok_or(WebError::BufferTooLong)
  }
}

#[op2]
fn op_base64_btoa<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[scoped] s: ByteString,
) -> Result<v8::Local<'a, v8::String>, WebError> {
  base64_encode_to_v8_string(
    scope,
    s.as_ref(),
    v8::simdutf::Base64Options::Default,
  )
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
    let written = simdutf_base64_encode(
      s,
      buf.as_mut_ptr(),
      b64_len,
      v8::simdutf::Base64Options::Default,
    );
    buf.set_len(written);
    String::from_utf8_unchecked(buf)
  }
}

// Base64url (RFC 4648 section 5, URL-safe alphabet, unpadded output) ops.
// These mirror the standard base64 ops above, sharing the parameterized
// simdutf helpers with the Url alphabet option. The Url option rejects the
// standard `+`/`/` alphabet on decode and emits unpadded output on encode.

#[op2]
fn op_base64url_decode(
  #[string(onebyte)] input: Cow<[u8]>,
) -> Result<Uint8Array, WebError> {
  let v =
    simdutf_base64_decode_to_vec(&input, v8::simdutf::Base64Options::Url)?;
  Ok(v.into())
}

/// Decode base64 into `target` with Loose last-chunk handling, truncating
/// when `target` is smaller than the decoded output. Returns the number of
/// bytes written, or -1 if the input is not valid base64 for the given
/// alphabet. Decode failure is a sentinel rather than an error so the JS
/// callers' cleaning fallbacks do not pay for a thrown exception on dirty
/// input. The count always fits i32: it is bounded by 3/4 of V8's maximum
/// string length.
#[inline]
fn base64_decode_into_slice(
  input: &[u8],
  target: &mut [u8],
  options: v8::simdutf::Base64Options,
) -> i32 {
  use v8::simdutf::LastChunkHandling;

  // Fast path: decode directly into target when it can hold the worst-case
  // decoded length (zero intermediate copies).
  let max_len = v8::simdutf::maximal_binary_length_from_base64(input);
  if target.len() >= max_len {
    return match simdutf_base64_decode_into(
      input,
      target,
      options,
      LastChunkHandling::Loose,
    ) {
      Some(len) => len as i32,
      None => -1,
    };
  }

  // Slow path: target is smaller than the decoded output (truncating
  // write); decode to scratch and copy what fits.
  const STACK_BUF_SIZE: usize = 8192;
  if max_len <= STACK_BUF_SIZE {
    let mut buf = std::mem::MaybeUninit::<[u8; STACK_BUF_SIZE]>::uninit();
    // Safety: simdutf writes into buf without reading uninitialized data.
    let decoded_len = match simdutf_base64_decode_into(
      input,
      unsafe {
        std::slice::from_raw_parts_mut(
          buf.as_mut_ptr() as *mut u8,
          STACK_BUF_SIZE,
        )
      },
      options,
      LastChunkHandling::Loose,
    ) {
      Some(len) => len,
      None => return -1,
    };
    let bytes_to_write = decoded_len.min(target.len());
    // Safety: decoded_len bytes were written by simdutf.
    target[..bytes_to_write].copy_from_slice(unsafe {
      std::slice::from_raw_parts(buf.as_ptr() as *const u8, bytes_to_write)
    });
    bytes_to_write as i32
  } else {
    let decoded = match simdutf_base64_decode_to_vec(input, options) {
      Ok(v) => v,
      Err(_) => return -1,
    };
    let bytes_to_write = decoded.len().min(target.len());
    target[..bytes_to_write].copy_from_slice(&decoded[..bytes_to_write]);
    bytes_to_write as i32
  }
}

/// Decode base64url into `target` at `offset`. Returns the number of bytes
/// written, or the -1 invalid-input sentinel (see base64_decode_into_slice).
///
/// Unlike op_base64_decode_into there is no strict pre-pass: base64url input
/// is typically unpadded and simdutf Strict rejects unpadded final chunks, so
/// the direct path decodes Loose straight into the target.
#[op2(fast)]
fn op_base64url_decode_into(
  #[string(onebyte)] input: Cow<[u8]>,
  #[buffer] target: &mut [u8],
  #[smi] offset: u32,
) -> Result<i32, WebError> {
  let offset = offset as usize;
  let target = target.get_mut(offset..).ok_or(WebError::BufferTooSmall)?;
  Ok(base64_decode_into_slice(
    &input,
    target,
    v8::simdutf::Base64Options::Url,
  ))
}

/// Encode a sub-range of a buffer to base64url, avoiding a JS-side slice copy.
#[op2]
fn op_base64url_encode_from_buffer<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[buffer] s: &[u8],
  #[smi] offset: u32,
  #[smi] length: u32,
) -> Result<v8::Local<'a, v8::String>, WebError> {
  let offset = offset as usize;
  let length = length as usize;
  let end = offset.checked_add(length).ok_or(WebError::BufferTooSmall)?;
  let s = s.get(offset..end).ok_or(WebError::BufferTooSmall)?;
  base64_encode_to_v8_string(scope, s, v8::simdutf::Base64Options::Url)
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

// Streaming-mode fast path for UTF-8 decoding: returns a V8 string when the
// input is pure ASCII, and `null` otherwise. Pure ASCII can never split a
// codepoint at a chunk boundary, so a streaming `TextDecoder` whose internal
// state is idle can decode an ASCII chunk without touching its incremental
// decoder at all. Used by `TextDecoder.decode(chunk, { stream: true })` in
// `08_text_encoding.js` to skip the `Vec<u16>` allocation and UTF-16
// conversion of the encoding_rs path while keeping the decoder idle for the
// next ASCII chunk.
#[op2]
fn op_encoding_decode_utf8_ascii_only<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[anybuffer] zero_copy: &[u8],
) -> Option<v8::Local<'a, v8::String>> {
  if !v8::simdutf::validate_ascii(zero_copy) {
    return None;
  }
  v8::String::new_from_one_byte(scope, zero_copy, v8::NewStringType::Normal)
}

#[op2]
fn op_encoding_decode_utf8<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[anybuffer] zero_copy: &[u8],
  ignore_bom: bool,
) -> Result<v8::Local<'a, v8::String>, WebError> {
  // ASCII fast path. Pure ASCII inputs (the dominant real-world case for
  // HTTP/JSON bodies, file reads, etc.) are valid UTF-8 with no BOM, so we
  // can short-circuit straight to `new_from_one_byte` and skip both the
  // 3-byte BOM check and V8's internal UTF-8 validation pass.
  // `simdutf::validate_ascii` is a SIMD high-bit scan (~1ns per 64 bytes).
  if v8::simdutf::validate_ascii(zero_copy) {
    return v8::String::new_from_one_byte(
      scope,
      zero_copy,
      v8::NewStringType::Normal,
    )
    .ok_or(WebError::BufferTooLong);
  }

  let buf = if !ignore_bom
    && zero_copy.len() >= 3
    && zero_copy[0] == 0xef
    && zero_copy[1] == 0xbb
    && zero_copy[2] == 0xbf
  {
    &zero_copy[3..]
  } else {
    zero_copy
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

#[allow(deprecated, reason = "uses a deprecated serde_v8 magic type; kept until call sites migrate")]
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

#[allow(deprecated, reason = "uses a deprecated serde_v8 magic type; kept until call sites migrate")]
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

const ENCODE_INTO_PACKED_SENTINEL: f64 = -1.0;
const ENCODE_INTO_MAX_PACKED_READ: usize = (1 << 21) - 1;
const ENCODE_INTO_PACKED_MULTIPLIER: f64 = (1u64 << 32) as f64;

#[inline]
fn pack_encode_into_result(read: usize, written: usize) -> f64 {
  debug_assert!(read <= ENCODE_INTO_MAX_PACKED_READ);
  debug_assert!(written <= u32::MAX as usize);
  (read as f64) * ENCODE_INTO_PACKED_MULTIPLIER + written as f64
}

fn write_encode_into_result(
  out_buf: &mut [u32],
  read: usize,
  written: usize,
) -> Result<(), WebError> {
  if out_buf.len() < 2 {
    return Err(WebError::BufferTooSmall);
  }
  out_buf[1] = written as u32;
  out_buf[0] = read as u32;
  Ok(())
}

#[op2(fast(op_encoding_encode_into_fast))]
fn op_encoding_encode_into(
  scope: &mut v8::PinScope<'_, '_>,
  input: v8::Local<v8::Value>,
  #[buffer] buffer: &mut [u8],
) -> Result<f64, WebError> {
  let s = v8::Local::<v8::String>::try_from(input)?;

  if s.length() > ENCODE_INTO_MAX_PACKED_READ
    && buffer.len() > ENCODE_INTO_MAX_PACKED_READ
  {
    return Ok(ENCODE_INTO_PACKED_SENTINEL);
  }

  let mut nchars = 0;
  let len = s.write_utf8_v2(
    scope,
    buffer,
    v8::WriteFlags::kReplaceInvalidUtf8,
    Some(&mut nchars),
  );

  debug_assert!(nchars <= ENCODE_INTO_MAX_PACKED_READ);
  debug_assert!(len <= u32::MAX as usize);
  Ok(pack_encode_into_result(nchars, len))
}

#[op2(fast)]
fn op_encoding_encode_into_fallback(
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
  write_encode_into_result(out_buf, nchars, len)
}

#[op2(fast)]
fn op_encoding_encode_into_fast(
  #[string] input: Cow<'_, str>,
  #[buffer] buffer: &mut [u8],
) -> f64 {
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

  // The `read` output parameter is measured in UTF-16 code units.
  let read = match input {
    // Borrowed Cow strings are zero-copy views into the V8 heap.
    // Thus, they are guarantee to be SeqOneByteString.
    Cow::Borrowed(v) => v[..boundary].len(),
    Cow::Owned(ref v) => v[..boundary].encode_utf16().count(),
  };

  if read > ENCODE_INTO_MAX_PACKED_READ || boundary > u32::MAX as usize {
    return ENCODE_INTO_PACKED_SENTINEL;
  }

  buffer[..boundary].copy_from_slice(input[..boundary].as_bytes());

  pack_encode_into_result(read, boundary)
}

pub struct Location(pub Url);

#[cfg(test)]
mod tests {
  use v8::simdutf::Base64Options;
  use v8::simdutf::LastChunkHandling;

  use super::WebError;
  use super::base64_decode_into_slice;
  use super::simdutf_base64_decode_into;
  use super::simdutf_base64_decode_to_vec;
  use super::simdutf_base64_encode;
  use super::v8;
  use super::write_encode_into_result;

  /// Test helper: encode to a base64 String with the given alphabet. The
  /// production encode path (base64_encode_to_v8_string) needs a V8 scope;
  /// this exercises the same FFI encode underneath.
  fn encode_to_string(s: &[u8], options: Base64Options) -> String {
    let b64_len = v8::simdutf::base64_length_from_binary(s.len(), options);
    let mut buf = Vec::with_capacity(b64_len);
    // Safety: buf has b64_len bytes of capacity; simdutf output is ASCII.
    unsafe {
      let written =
        simdutf_base64_encode(s, buf.as_mut_ptr(), b64_len, options);
      buf.set_len(written);
      String::from_utf8_unchecked(buf)
    }
  }

  fn base64url_encode(s: &[u8]) -> String {
    encode_to_string(s, Base64Options::Url)
  }

  fn base64url_decode_to_vec(input: &[u8]) -> Result<Vec<u8>, WebError> {
    simdutf_base64_decode_to_vec(input, Base64Options::Url)
  }

  fn base64url_decode_into(input: &[u8], output: &mut [u8]) -> Option<usize> {
    simdutf_base64_decode_into(
      input,
      output,
      Base64Options::Url,
      LastChunkHandling::Loose,
    )
  }

  fn base64url_decode_into_slice(input: &[u8], target: &mut [u8]) -> i32 {
    base64_decode_into_slice(input, target, Base64Options::Url)
  }

  // RFC 4648 section 10 test vectors, base64url form (unpadded).
  // Covers all len % 3 encode classes and all valid len % 4 decode classes.
  const RFC4648_VECTORS: &[(&[u8], &str)] = &[
    (b"", ""),
    (b"f", "Zg"),
    (b"fo", "Zm8"),
    (b"foo", "Zm9v"),
    (b"foob", "Zm9vYg"),
    (b"fooba", "Zm9vYmE"),
    (b"foobar", "Zm9vYmFy"),
  ];

  #[test]
  fn base64url_encode_rfc4648_vectors() {
    for (input, expected) in RFC4648_VECTORS {
      assert_eq!(&base64url_encode(input), expected);
    }
  }

  #[test]
  fn base64url_encode_uses_url_alphabet_unpadded() {
    // 0xfb 0xff encodes to "+/8=" in standard base64.
    assert_eq!(base64url_encode(&[0xfb, 0xff]), "-_8");
    let all_bytes: Vec<u8> = (0..=255).collect();
    let encoded = base64url_encode(&all_bytes);
    assert!(!encoded.contains(['+', '/', '=']));
  }

  #[test]
  fn base64_std_encode_padded_standard_alphabet() {
    // Guards the alphabet parameterization: Default pads and keeps `+`/`/`.
    assert_eq!(
      encode_to_string(&[0xfb, 0xff], Base64Options::Default),
      "+/8="
    );
    assert_eq!(encode_to_string(b"f", Base64Options::Default), "Zg==");
    assert_eq!(
      encode_to_string(b"foobar", Base64Options::Default),
      "Zm9vYmFy"
    );
  }

  #[test]
  fn base64_std_decode_into_strict_and_loose() {
    // Strict accepts only clean padded input; Loose additionally accepts
    // missing padding. Both reject the url alphabet.
    let mut buf = [0u8; 8];
    assert_eq!(
      simdutf_base64_decode_into(
        b"Zm9vYg==",
        &mut buf,
        Base64Options::Default,
        LastChunkHandling::Strict,
      ),
      Some(4)
    );
    assert_eq!(&buf[..4], b"foob");
    assert!(
      simdutf_base64_decode_into(
        b"Zm9vYg",
        &mut buf,
        Base64Options::Default,
        LastChunkHandling::Strict,
      )
      .is_none()
    );
    assert_eq!(
      simdutf_base64_decode_into(
        b"Zm9vYg",
        &mut buf,
        Base64Options::Default,
        LastChunkHandling::Loose,
      ),
      Some(4)
    );
    assert!(
      simdutf_base64_decode_into(
        b"-_8",
        &mut buf,
        Base64Options::Default,
        LastChunkHandling::Loose,
      )
      .is_none()
    );
  }

  #[test]
  #[should_panic]
  fn base64_decode_into_asserts_output_capacity() {
    // The capacity assert is a memory-safety guard: simdutf may write up to
    // the maximal decoded length before detecting an error.
    let mut small = [0u8; 1];
    let _ = simdutf_base64_decode_into(
      b"Zm9vYg==",
      &mut small,
      Base64Options::Default,
      LastChunkHandling::Loose,
    );
  }

  #[test]
  fn base64url_encode_decode_64kib_round_trip() {
    let data: Vec<u8> = (0..65536u32).map(|i| ((i * 31) >> 3) as u8).collect();
    let encoded = base64url_encode(&data);
    // 65536 = 3 * 21845 + 1 -> 21845 full quads + 2 chars, no padding.
    assert_eq!(encoded.len(), 21845 * 4 + 2);
    let decoded = base64url_decode_to_vec(encoded.as_bytes()).unwrap();
    assert_eq!(decoded, data);
  }

  #[test]
  fn base64url_decode_accepts_padded_and_unpadded() {
    for (expected, unpadded) in RFC4648_VECTORS {
      assert_eq!(
        &base64url_decode_to_vec(unpadded.as_bytes()).unwrap(),
        expected
      );
      let mut padded = (*unpadded).to_string();
      while padded.len() % 4 != 0 {
        padded.push('=');
      }
      assert_eq!(
        &base64url_decode_to_vec(padded.as_bytes()).unwrap(),
        expected
      );
    }
    assert_eq!(base64url_decode_to_vec(b"-_8").unwrap(), [0xfb, 0xff]);
  }

  #[test]
  fn base64url_decode_rejects_invalid_input() {
    // Standard alphabet is not accepted by the Url option.
    assert!(base64url_decode_to_vec(b"+_8").is_err());
    assert!(base64url_decode_to_vec(b"a/b0").is_err());
    // Junk characters.
    assert!(base64url_decode_to_vec(b"!!!!").is_err());
    // len % 4 == 1 residue cannot encode a byte.
    assert!(base64url_decode_to_vec(b"Zm9vY").is_err());
    // Oversized and misplaced padding.
    assert!(base64url_decode_to_vec(b"QQ===").is_err());
    assert!(base64url_decode_to_vec(b"QQ=").is_err());
    assert!(base64url_decode_to_vec(b"QQ==QQ==").is_err());
  }

  #[test]
  fn base64url_decode_strips_ascii_whitespace() {
    // Loose mode strips whitespace anywhere in the input.
    assert_eq!(base64url_decode_to_vec(b"Zm \t9v\n").unwrap(), b"foo");
  }

  #[test]
  fn base64url_decode_into_exact_fit_and_oversized_target() {
    let mut exact = [0u8; 3];
    let n = base64url_decode_into(b"Zm9v", &mut exact).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&exact, b"foo");

    let mut oversized = [0xaau8; 16];
    let n = base64url_decode_into(b"Zm8", &mut oversized).unwrap();
    assert_eq!(n, 2);
    assert_eq!(&oversized[..2], b"fo");
    assert_eq!(oversized[15], 0xaa);

    let mut empty = [0u8; 0];
    assert_eq!(base64url_decode_into(b"", &mut empty).unwrap(), 0);

    let mut buf = [0u8; 4];
    assert!(base64url_decode_into(b"++++", &mut buf).is_none());
  }

  #[test]
  fn base64url_decode_into_slice_direct_and_truncating() {
    // Direct path: target holds the worst-case decoded length.
    let mut exact = [0u8; 3];
    assert_eq!(base64url_decode_into_slice(b"Zm9v", &mut exact), 3);
    assert_eq!(&exact, b"foo");

    // Stack scratch path: max_len <= 8192, target smaller than the output.
    let data: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
    let encoded = base64url_encode(&data);
    let mut small = [0u8; 100];
    assert_eq!(
      base64url_decode_into_slice(encoded.as_bytes(), &mut small),
      100
    );
    assert_eq!(&small[..], &data[..100]);

    // Vec scratch path: max_len > 8192.
    let data: Vec<u8> = (0..16384u32).map(|i| (i % 249) as u8).collect();
    let encoded = base64url_encode(&data);
    let mut small = [0u8; 1000];
    assert_eq!(
      base64url_decode_into_slice(encoded.as_bytes(), &mut small),
      1000
    );
    assert_eq!(&small[..], &data[..1000]);
  }

  #[test]
  fn base64url_decode_into_slice_scratch_boundary() {
    // 8192 decoded bytes encode to 10923 chars: max_len == STACK_BUF_SIZE,
    // the last input handled on the stack. 8193 bytes (10924 chars) is the
    // first to take the Vec path. Truncating targets force the scratch path.
    for size in [8192usize, 8193] {
      let data: Vec<u8> = (0..size).map(|i| (i % 247) as u8).collect();
      let encoded = base64url_encode(&data);
      let mut target = vec![0u8; size - 1];
      assert_eq!(
        base64url_decode_into_slice(encoded.as_bytes(), &mut target),
        (size - 1) as i32
      );
      assert_eq!(&target[..], &data[..size - 1]);
    }
  }

  #[test]
  fn base64url_decode_into_slice_sentinel_on_all_paths() {
    // Direct path.
    assert_eq!(base64url_decode_into_slice(b"!!!!", &mut [0u8; 16]), -1);
    // Stack scratch path (target smaller than max_len).
    assert_eq!(base64url_decode_into_slice(b"!!!!!!!!", &mut [0u8; 2]), -1);
    // Vec scratch path (max_len > 8192).
    let mut big = vec![b'A'; 12000];
    big.push(b'!');
    assert_eq!(base64url_decode_into_slice(&big, &mut [0u8; 4]), -1);
  }

  #[test]
  fn base64_std_decode_into_slice_direct_and_truncating() {
    let std = Base64Options::Default;
    // Direct path with padded input.
    let mut exact = [0u8; 3];
    assert_eq!(base64_decode_into_slice(b"Zm9v", &mut exact, std), 3);
    assert_eq!(&exact, b"foo");

    // Truncating write via the scratch path; whitespace forces Loose.
    let mut small = [0u8; 2];
    assert_eq!(base64_decode_into_slice(b"Zm9v YmFy", &mut small, std), 2);
    assert_eq!(&small, b"fo");
  }

  #[test]
  fn base64_std_decode_into_slice_sentinel_on_all_paths() {
    let std = Base64Options::Default;
    // Direct path.
    assert_eq!(base64_decode_into_slice(b"!!!!", &mut [0u8; 16], std), -1);
    // The url alphabet is invalid for the standard decode.
    assert_eq!(base64_decode_into_slice(b"-_8", &mut [0u8; 16], std), -1);
    // Stack scratch path (target smaller than max_len).
    assert_eq!(
      base64_decode_into_slice(b"!!!!!!!!", &mut [0u8; 2], std),
      -1
    );
    // Vec scratch path (max_len > 8192).
    let mut big = vec![b'A'; 12000];
    big.push(b'!');
    assert_eq!(base64_decode_into_slice(&big, &mut [0u8; 4], std), -1);
  }

  #[test]
  fn base64url_strict_rejects_unpadded_final_chunk() {
    // Pins why op_base64url_decode_into decodes Loose on its direct path:
    // Strict errors on the unpadded final chunk that dominates real
    // base64url input, while Loose accepts both padded and unpadded forms.
    use v8::simdutf;
    let mut buf = [0u8; 8];
    // Safety: buf is larger than the maximal decoded length of the inputs.
    let unpadded = unsafe {
      simdutf::base64_to_binary(
        b"Zm9vYg",
        &mut buf,
        simdutf::Base64Options::Url,
        simdutf::LastChunkHandling::Strict,
      )
    };
    assert!(!unpadded.is_ok());
    // Safety: buf is larger than the maximal decoded length of the inputs.
    let padded = unsafe {
      simdutf::base64_to_binary(
        b"Zm9vYg==",
        &mut buf,
        simdutf::Base64Options::Url,
        simdutf::LastChunkHandling::Strict,
      )
    };
    assert!(padded.is_ok());
    assert_eq!(padded.count, 4);
  }

  #[test]
  fn encode_into_result_rejects_undersized_output_buffer() {
    let mut empty: [u32; 0] = [];
    assert!(matches!(
      write_encode_into_result(&mut empty, 1, 1),
      Err(WebError::BufferTooSmall)
    ));

    let mut one = [0];
    assert!(matches!(
      write_encode_into_result(&mut one, 1, 1),
      Err(WebError::BufferTooSmall)
    ));
    assert_eq!(one, [0]);
  }

  #[test]
  fn encode_into_result_writes_read_and_written_counts() {
    let mut out = [0, 0];
    write_encode_into_result(&mut out, 3, 7).unwrap();
    assert_eq!(out, [3, 7]);
  }
}
