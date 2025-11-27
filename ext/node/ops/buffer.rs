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

#[op2(fast)]
#[smi]
pub fn op_node_buffer_compare(
  #[buffer] buf1: &[u8],
  #[buffer] buf2: &[u8],
) -> i32 {
  buf1.cmp(buf2) as i32
}

#[op2(fast)]
#[smi]
pub fn op_node_buffer_compare_offset(
  #[buffer] source: &[u8],
  #[buffer] target: &[u8],
  #[smi] source_start: usize,
  #[smi] target_start: usize,
  #[smi] source_end: usize,
  #[smi] target_end: usize,
) -> Result<i32, JsErrorBox> {
  if source_start > source.len() {
    return Err(JsErrorBox::from_err(BufferError::OutOfRangeNamed(
      "sourceStart".to_string(),
    )));
  }
  if target_start > target.len() {
    return Err(JsErrorBox::from_err(BufferError::OutOfRangeNamed(
      "targetStart".to_string(),
    )));
  }

  if source_start > source_end {
    panic!("source_start > source_end");
  }
  if target_start > target_end {
    panic!("target_start > target_end");
  }

  Ok(
    source[source_start..source_end].cmp(&target[target_start..target_end])
      as i32,
  )
}

#[op2]
pub fn op_node_decode_utf8<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  buf: v8::Local<v8::ArrayBufferView>,
  start: v8::Local<v8::Value>,
  end: v8::Local<v8::Value>,
) -> Result<v8::Local<'a, v8::String>, JsErrorBox> {
  let buf = buf.get_contents(&mut [0; v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP]);

  let start =
    parse_array_index(scope, start, 0).map_err(JsErrorBox::from_err)?;
  let mut end =
    parse_array_index(scope, end, buf.len()).map_err(JsErrorBox::from_err)?;

  if end < start {
    end = start;
  }

  if end > buf.len() {
    return Err(JsErrorBox::from_err(BufferError::OutOfRange));
  }

  let buffer = &buf[start..end];

  if buffer.len() <= 256 && buffer.is_ascii() {
    v8::String::new_from_one_byte(scope, buffer, v8::NewStringType::Normal)
      .ok_or_else(|| JsErrorBox::from_err(BufferError::StringTooLong))
  } else {
    v8::String::new_from_utf8(scope, buffer, v8::NewStringType::Normal)
      .ok_or_else(|| JsErrorBox::from_err(BufferError::StringTooLong))
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
enum BufferError {
  #[error(
    "Cannot create a string longer than 0x{:x} characters",
    v8::String::MAX_LENGTH
  )]
  #[class(generic)]
  #[property("code" = "ERR_STRING_TOO_LONG")]
  StringTooLong,
  #[error("Invalid type")]
  #[class(generic)]
  InvalidType,
  #[error("Index out of range")]
  #[class(range)]
  #[property("code" = "ERR_OUT_OF_RANGE")]
  OutOfRange,
  #[error("The value of \"{0}\" is out of range.")]
  #[class(range)]
  #[property("code" = "ERR_OUT_OF_RANGE")]
  OutOfRangeNamed(String),
}

#[inline(always)]
fn parse_array_index(
  scope: &mut v8::PinScope<'_, '_>,
  arg: v8::Local<v8::Value>,
  default: usize,
) -> Result<usize, BufferError> {
  if arg.is_undefined() {
    return Ok(default);
  }

  let Some(arg) = arg.integer_value(scope) else {
    return Err(BufferError::InvalidType);
  };
  if arg < 0 {
    return Err(BufferError::OutOfRange);
  }
  if arg > isize::MAX as i64 {
    return Err(BufferError::OutOfRange);
  }
  Ok(arg as usize)
}
