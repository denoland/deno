// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;

#[op2(fast)]
pub fn op_is_ascii(#[buffer] buf: &[u8]) -> bool {
  buf.is_ascii()
}

#[op2(fast)]
pub fn op_is_utf8(#[buffer] buf: &[u8]) -> bool {
  std::str::from_utf8(buf).is_ok()
}

#[op2]
#[buffer]
pub fn op_transcode(
  #[buffer] source: &[u8],
  #[string] from_encoding: &str,
  #[string] to_encoding: &str,
) -> Result<Vec<u8>, JsErrorBox> {
  match (from_encoding, to_encoding) {
    ("utf8", "ascii") => Ok(utf8_to_ascii(source)),
    ("utf8", "latin1") => Ok(utf8_to_latin1(source)),
    ("utf8", "utf16le") => utf8_to_utf16le(source),
    ("utf16le", "utf8") => utf16le_to_utf8(source),
    ("latin1", "utf16le") | ("ascii", "utf16le") => {
      Ok(latin1_ascii_to_utf16le(source))
    }
    (from, to) => Err(JsErrorBox::generic(format!(
      "Unable to transcode Buffer {from}->{to}"
    ))),
  }
}

fn latin1_ascii_to_utf16le(source: &[u8]) -> Vec<u8> {
  let mut result = Vec::with_capacity(source.len() * 2);
  for &byte in source {
    result.push(byte);
    result.push(0);
  }
  result
}

fn utf16le_to_utf8(source: &[u8]) -> Result<Vec<u8>, JsErrorBox> {
  let ucs2_vec: Vec<u16> = source
    .chunks(2)
    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
    .collect();
  String::from_utf16(&ucs2_vec)
    .map(|utf8_string| utf8_string.into_bytes())
    .map_err(|e| JsErrorBox::generic(format!("Invalid UTF-16 sequence: {}", e)))
}

fn utf8_to_utf16le(source: &[u8]) -> Result<Vec<u8>, JsErrorBox> {
  let utf8_string =
    std::str::from_utf8(source).map_err(JsErrorBox::from_err)?;
  let ucs2_vec: Vec<u16> = utf8_string.encode_utf16().collect();
  let bytes: Vec<u8> = ucs2_vec.iter().flat_map(|&x| x.to_le_bytes()).collect();
  Ok(bytes)
}

fn utf8_to_latin1(source: &[u8]) -> Vec<u8> {
  let mut latin1_bytes = Vec::with_capacity(source.len());
  let mut i = 0;
  while i < source.len() {
    match source[i] {
      byte if byte <= 0x7F => {
        // ASCII character
        latin1_bytes.push(byte);
        i += 1;
      }
      byte if (0xC2..=0xDF).contains(&byte) && i + 1 < source.len() => {
        // 2-byte UTF-8 sequence
        let codepoint =
          ((byte as u16 & 0x1F) << 6) | (source[i + 1] as u16 & 0x3F);
        latin1_bytes.push(if codepoint <= 0xFF {
          codepoint as u8
        } else {
          b'?'
        });
        i += 2;
      }
      _ => {
        // 3-byte or 4-byte UTF-8 sequence, or invalid UTF-8
        latin1_bytes.push(b'?');
        // Skip to the next valid UTF-8 start byte
        i += 1;
        while i < source.len() && (source[i] & 0xC0) == 0x80 {
          i += 1;
        }
      }
    }
  }
  latin1_bytes
}

fn utf8_to_ascii(source: &[u8]) -> Vec<u8> {
  let mut ascii_bytes = Vec::with_capacity(source.len());
  let mut i = 0;
  while i < source.len() {
    match source[i] {
      byte if byte <= 0x7F => {
        // ASCII character
        ascii_bytes.push(byte);
        i += 1;
      }
      _ => {
        // Non-ASCII character
        ascii_bytes.push(b'?');
        // Skip to the next valid UTF-8 start byte
        i += 1;
        while i < source.len() && (source[i] & 0xC0) == 0x80 {
          i += 1;
        }
      }
    }
  }
  ascii_bytes
}

#[op2]
pub fn op_node_unsafe_decode_utf8<'a>(
  scope: &mut v8::HandleScope<'a>,
  buf: v8::Local<v8::Value>,
  byte_offset: v8::Local<v8::Value>,
  byte_length: v8::Local<v8::Value>,
  start: v8::Local<v8::Value>,
  end: v8::Local<v8::Value>,
) -> Result<v8::Local<'a, v8::String>, JsErrorBox> {
  // SAFETY: the javascript side must guarantee that the arguments are of the correct types
  let buf = unsafe { v8::Local::<v8::ArrayBuffer>::cast_unchecked(buf) };
  // SAFETY: upheld by js
  let byte_offset = unsafe { to_usize_unchecked(byte_offset) };
  // SAFETY: upheld by js
  let byte_length = unsafe { to_usize_unchecked(byte_length) };
  // SAFETY: upheld by js
  let start = unsafe { to_usize_unchecked(start) };
  // SAFETY: upheld by js
  let end = unsafe { to_usize_unchecked(end) };

  // SAFETY: the javascript side must guarantee that the arguments are in bounds
  let buffer = unsafe { buffer_to_slice(&buf, byte_offset, byte_length) };
  let buffer = &buffer[start..end];

  if buffer.len() <= 256 && buffer.is_ascii() {
    v8::String::new_from_one_byte(scope, buffer, v8::NewStringType::Normal)
      .ok_or_else(|| JsErrorBox::from_err(BufferError::StringTooLong))
  } else {
    v8::String::new_from_utf8(scope, buffer, v8::NewStringType::Normal)
      .ok_or_else(|| JsErrorBox::from_err(BufferError::StringTooLong))
  }
}

/// # Safety
///
/// The caller must guarantee that the argument is a valid `v8::Number`.
unsafe fn to_usize_unchecked(arg: v8::Local<v8::Value>) -> usize {
  // SAFETY: checked by caller
  let arg = unsafe { v8::Local::<v8::Number>::cast_unchecked(arg) };
  arg.value() as usize
}

/// # Safety
///
/// The caller must guarantee that byte_offset and byte_length are valid.
unsafe fn buffer_to_slice<'a>(
  buf: &'a v8::Local<v8::ArrayBuffer>,
  byte_offset: usize,
  byte_length: usize,
) -> &'a [u8] {
  let Some(ptr) = buf.data() else {
    return &[];
  };
  // SAFETY: caller
  unsafe {
    let ptr = ptr.cast::<u8>().add(byte_offset);
    std::slice::from_raw_parts(ptr.as_ptr(), byte_length)
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
enum BufferError {
  #[error("String too long")]
  #[class(generic)]
  #[property("code" = "ERR_STRING_TOO_LONG")]
  StringTooLong,
}
