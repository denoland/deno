// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::convert::Uint8Array;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::ExternalReference;
use deno_core::v8::MapFnTo;
use deno_core::v8_static_strings;
use deno_error::JsErrorBox;

#[op2(fast)]
pub fn op_mark_as_untransferable(
  scope: &mut v8::PinScope<'_, '_>,
  ab: v8::Local<v8::ArrayBuffer>,
) {
  v8_static_strings! {
      UNTRANSFERABLE = "untransferable",
  }

  let key = UNTRANSFERABLE.v8_string(scope).unwrap();
  ab.set_detach_key(key.into());
}

#[op2(fast)]
pub fn op_is_ascii(#[buffer] buf: &[u8]) -> bool {
  buf.is_ascii()
}

#[op2(fast)]
pub fn op_is_utf8(#[buffer] buf: &[u8]) -> bool {
  std::str::from_utf8(buf).is_ok()
}

#[op2]
pub fn op_transcode(
  #[buffer] source: &[u8],
  #[string] from_encoding: &str,
  #[string] to_encoding: &str,
) -> Result<Uint8Array, JsErrorBox> {
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

fn latin1_ascii_to_utf16le(source: &[u8]) -> Uint8Array {
  let mut result = Vec::with_capacity(source.len() * 2);
  for &byte in source {
    result.push(byte);
    result.push(0);
  }
  result.into()
}

fn utf16le_to_utf8(source: &[u8]) -> Result<Uint8Array, JsErrorBox> {
  let ucs2_vec: Vec<u16> = source
    .chunks_exact(2)
    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
    .collect();
  String::from_utf16(&ucs2_vec)
    .map(|utf8_string| utf8_string.into_bytes().into())
    .map_err(|e| JsErrorBox::generic(format!("Invalid UTF-16 sequence: {}", e)))
}

fn utf8_to_utf16le(source: &[u8]) -> Result<Uint8Array, JsErrorBox> {
  let utf8_string =
    std::str::from_utf8(source).map_err(JsErrorBox::from_err)?;
  let ucs2_vec: Vec<u16> = utf8_string.encode_utf16().collect();
  let bytes: Vec<u8> = ucs2_vec.iter().flat_map(|&x| x.to_le_bytes()).collect();
  Ok(bytes.into())
}

fn utf8_to_latin1(source: &[u8]) -> Uint8Array {
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
  latin1_bytes.into()
}

fn utf8_to_ascii(source: &[u8]) -> Uint8Array {
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
  ascii_bytes.into()
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

// Threshold for falling back to V8's internal string copy allocation
// instead of creating an ExternalString to reduce GC finalizer overhead.
const ZERO_COPY_THRESHOLD: usize = 1024;

#[op2(reentrant)]
pub fn op_node_encoding_slice<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  buf: v8::Local<v8::ArrayBufferView>,
  start: v8::Local<v8::Value>,
  end: v8::Local<v8::Value>,
  encoding: u8,
) -> Result<v8::Local<'a, v8::String>, JsErrorBox> {
  let buf_len = buf.byte_length();

  let start =
    parse_array_index(scope, start, 0).map_err(JsErrorBox::from_err)?;
  let mut end =
    parse_array_index(scope, end, buf_len).map_err(JsErrorBox::from_err)?;

  let mut storage = [0; v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP];
  let buf = buf.get_contents(&mut storage);

  if end < start {
    end = start;
  }

  if end > buf.len() {
    return Err(JsErrorBox::from_err(BufferError::OutOfRange));
  }

  if end == start {
    return Ok(v8::String::empty(scope));
  }

  let buffer = &buf[start..end];

  match encoding {
    0 => {
      // utf8Slice
      if buffer.len() <= 256 && buffer.is_ascii() {
        v8::String::new_from_one_byte(scope, buffer, v8::NewStringType::Normal)
      } else {
        // `v8::String::new_from_utf8` does not replace ill-formed UTF-8
        // sequences the same way Node.js does, so decode lossily ourselves.
        // Rust's `from_utf8_lossy` follows the WHATWG "maximal subpart"
        // replacement (one U+FFFD per ill-formed subsequence), matching Node's
        // `Buffer.prototype.toString('utf8')` and `string_decoder`. For valid
        // UTF-8 it borrows the input without allocating.
        let decoded = String::from_utf8_lossy(buffer);
        v8::String::new_from_utf8(
          scope,
          decoded.as_bytes(),
          v8::NewStringType::Normal,
        )
      }
    }
    1 => {
      // latin1Slice
      v8::String::new_from_one_byte(scope, buffer, v8::NewStringType::Normal)
    }
    2 => {
      // asciiSlice
      if buffer.len() > v8::String::MAX_LENGTH {
        // String too long
        None
      } else if buffer.len() > ZERO_COPY_THRESHOLD {
        let ascii_bytes = mask_ascii_fast(buffer);
        // `ascii_bytes` is already a copied clone
        // (not a zero‑copy reference to the ArrayBufferView),
        // so we can zero‑copy create a V8 string from it.
        v8::String::new_external_onebyte(scope, ascii_bytes.into_boxed_slice())
      } else if buffer.is_ascii() {
        // A copy is required to prevent subsequent ArrayBufferView modifications
        // from altering the immutable string.
        // Cannot zero-copy create a V8 string here.
        v8::String::new_from_one_byte(scope, buffer, v8::NewStringType::Normal)
      } else {
        let ascii_bytes = mask_ascii_fast(buffer);
        // Copy bytes to a string
        v8::String::new_from_one_byte(
          scope,
          &ascii_bytes,
          v8::NewStringType::Normal,
        )
      }
    }
    3 => {
      // ucs2Slice
      decode_utf16le_from_bytes(scope, buffer)
    }
    4 => {
      // hexSlice
      if buffer.len() > (v8::String::MAX_LENGTH / 2) {
        // String too long
        None
      } else {
        let target_len = buffer.len() * 2;
        let mut hex_bytes = vec![0u8; target_len];
        // infallible: output is exactly 2x input
        faster_hex::hex_encode(buffer, &mut hex_bytes).unwrap();
        if target_len <= ZERO_COPY_THRESHOLD {
          // Copy bytes to a string
          v8::String::new_from_one_byte(
            scope,
            &hex_bytes,
            v8::NewStringType::Normal,
          )
        } else {
          // Create a V8 string with zero-copy
          v8::String::new_external_onebyte(scope, hex_bytes.into_boxed_slice())
        }
      }
    }
    _ => return Err(JsErrorBox::from_err(BufferError::InvalidType)),
  }
  .ok_or_else(|| JsErrorBox::from_err(BufferError::StringTooLong))
}

#[inline(always)]
fn mask_ascii_fast(bytes: &[u8]) -> Vec<u8> {
  const CHUNK_SIZE: usize = std::mem::size_of::<usize>();
  const MASK: usize = usize::from_ne_bytes([0x7F; CHUNK_SIZE]);

  let len = bytes.len();
  let mut ascii_bytes = Vec::<u8>::with_capacity(len);

  let src = bytes.as_ptr();
  let dst = ascii_bytes.as_mut_ptr();

  // SAFETY:
  // 1. Bounds & Capacity:
  //    - `src` is valid for `len` bytes.
  //    - `dst` has an allocated capacity of `len` bytes.
  //    - If `len >= CHUNK_SIZE`: `i < limit` implies
  //      `i + CHUNK_SIZE < len`. The out-of-loop block at `limit`
  //      accesses exactly the last `CHUNK_SIZE` bytes.
  //    - If `len < CHUNK_SIZE`: The `for` loop bounds are `0..len`.
  //    Therefore, all pointer arithmetic stays within valid bounds.
  // 2. Alignment:
  //    `read_unaligned` and `write_unaligned` are used for `usize`
  //    accesses, preventing UB from potentially unaligned pointers.
  // 3. Initialization:
  //    Every byte from `0` to `len` in `dst` is guaranteed to be
  //    written before `set_len(len)` is called. Overlapping writes
  //    are idempotent and safe.
  unsafe {
    if len >= CHUNK_SIZE {
      let limit = len - CHUNK_SIZE;
      let mut i: usize = 0;
      while i < limit {
        let tmp = src.add(i).cast::<usize>().read_unaligned();
        dst.add(i).cast::<usize>().write_unaligned(tmp & MASK);
        i += CHUNK_SIZE;
      }
      let tmp = src.add(limit).cast::<usize>().read_unaligned();
      dst.add(limit).cast::<usize>().write_unaligned(tmp & MASK);
    } else {
      for i in 0..len {
        dst.add(i).write(src.add(i).read() & 0x7F);
      }
    }

    ascii_bytes.set_len(len);
  }

  ascii_bytes
}

#[inline(always)]
fn decode_utf16le_from_bytes<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  bytes: &[u8],
) -> Option<v8::Local<'a, v8::String>> {
  // UTF-16 must be a multiple of 2 bytes. Discard any trailing odd byte.
  let len = bytes.len() & !1;
  let target_len = len / 2;

  if target_len > v8::String::MAX_LENGTH {
    // String too long
    return None;
  }

  let buf = &bytes[..len];

  #[cfg(target_endian = "little")]
  {
    // Attempt a zero-copy cast to &[u16]
    // SAFETY:
    // `u16` has no invalid bit patterns. Reinterpreting
    // any initialized `u8` pairs as `u16` is safe.
    let (prefix, u16_slice, suffix) = unsafe { buf.align_to::<u16>() };

    if prefix.is_empty() && suffix.is_empty() {
      // Fast path: Memory is perfectly 2-byte aligned.
      // A copy is required to prevent subsequent ArrayBufferView modifications
      // from altering the immutable string.
      // Cannot zero-copy create a V8 string here.
      v8::String::new_from_two_byte(scope, u16_slice, v8::NewStringType::Normal)
    } else {
      // Slow path: Unaligned memory (rare in V8, but must be handled).
      // Use uninitialized memory to avoid Vec's memset(0) overhead.
      let mut u16_data = Vec::<u16>::with_capacity(target_len);

      // SAFETY:
      // 1. `buf` is valid for reads of `len` bytes.
      // 2. `u16_data` has a capacity of `target_len`
      //    `u16`s (exactly `len` bytes), so writing
      //    `len` bytes is within bounds.
      // 3. Source and destination do not overlap.
      // 4. `copy_nonoverlapping` fully initializes
      //    the memory, making `set_len` safe.
      unsafe {
        // Memcpy the data byte-by-byte into the newly allocated Vec memory.
        std::ptr::copy_nonoverlapping(
          buf.as_ptr(),
          u16_data.as_mut_ptr().cast::<u8>(),
          len,
        );
        // Manually set the length.
        u16_data.set_len(target_len);
      }

      // `u16_data` is already a copied clone
      // (not a zero‑copy reference to the ArrayBufferView),
      // so we can zero‑copy create a V8 string from it.
      if len <= ZERO_COPY_THRESHOLD {
        // Copy bytes to a string
        v8::String::new_from_two_byte(
          scope,
          &u16_data,
          v8::NewStringType::Normal,
        )
      } else {
        // Create a V8 string with zero-copy
        v8::String::new_external_twobyte(scope, u16_data.into_boxed_slice())
      }
    }
  }

  // Fallback for big-endian architectures (uncommon environments).
  #[cfg(target_endian = "big")]
  {
    let u16_data = buf
      .chunks_exact(2)
      .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
      .collect();

    // `u16_data` is already a copied clone
    // (not a zero‑copy reference to the ArrayBufferView),
    // so we can zero‑copy create a V8 string from it.
    if len <= ZERO_COPY_THRESHOLD {
      // Copy bytes to a string
      v8::String::new_from_two_byte(scope, &u16_data, v8::NewStringType::Normal)
    } else {
      // Create a V8 string with zero-copy
      v8::String::new_external_twobyte(scope, u16_data.into_boxed_slice())
    }
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

fn set_value(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: v8::Local<v8::Value>,
) {
  let key = v8::String::new(scope, name).unwrap();
  obj.set(scope, key.into(), value);
}

fn set_function(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  export_name: &str,
  function: v8::Local<v8::Function>,
) {
  let name = v8::String::new(scope, export_name).unwrap();
  function.set_name(name);
  set_value(scope, obj, export_name, function.into());
}

fn throw_error(scope: &mut v8::PinScope, message: &str) {
  let message = v8::String::new(scope, message).unwrap();
  let exception = v8::Exception::error(scope, message);
  scope.throw_exception(exception);
}

fn throw_type_error(scope: &mut v8::PinScope, message: &str) {
  let message = v8::String::new(scope, message).unwrap();
  let exception = v8::Exception::type_error(scope, message);
  scope.throw_exception(exception);
}

fn view_bytes(
  scope: &mut v8::PinScope,
  value: v8::Local<v8::Value>,
  name: &str,
) -> Option<Vec<u8>> {
  if !value.is_array_buffer_view() {
    throw_type_error(scope, &format!("{name} must be an ArrayBufferView"));
    return None;
  }
  let view = v8::Local::<v8::ArrayBufferView>::try_from(value).unwrap();
  let mut bytes = vec![0; view.byte_length()];
  let copied = view.copy_contents(&mut bytes);
  debug_assert_eq!(copied, bytes.len());
  Some(bytes)
}

fn index_of_needle(
  source: &[u8],
  needle: &[u8],
  mut start: i64,
  step: usize,
) -> i64 {
  let source_len = source.len() as i64;
  if start >= source_len {
    return -1;
  }
  if start < 0 {
    start = 0.max(source_len + start);
  }
  let start = start as usize;
  if needle.is_empty() {
    return start as i64;
  }
  if needle.len() > source.len() {
    return -1;
  }
  let first = needle[0];
  let mut i = start;
  while i < source.len() {
    if source[i] == first
      && i + needle.len() <= source.len()
      && source[i..i + needle.len()] == *needle
    {
      return i as i64;
    }
    i += step;
  }
  -1
}

fn slice_end(len: usize, end: i64) -> usize {
  if end < 0 {
    (len as i64 + end).max(0) as usize
  } else {
    (end as usize).min(len)
  }
}

fn find_last_index(target: &[u8], needle: &[u8], mut offset: i64) -> i64 {
  let target_len = target.len();
  let needle_len = needle.len();
  if offset > target_len as i64 {
    offset = target_len as i64;
  }
  let searchable_len = slice_end(target_len, offset + needle_len as i64);
  if needle_len == 0 {
    return searchable_len as i64;
  }
  if needle_len > searchable_len {
    return -1;
  }
  for index in (0..=searchable_len - needle_len).rev() {
    if target[index..index + needle_len] == *needle {
      return index as i64;
    }
  }
  -1
}

fn index_of_buffer_from_bytes(
  target: &[u8],
  needle: &[u8],
  mut byte_offset: i64,
  encoding: i32,
  forward: bool,
) -> i64 {
  let target_len = target.len() as i64;
  if encoding == 3 && (needle.len() < 2 || target.len() < 2) {
    return -1;
  }
  if !forward {
    if byte_offset < 0 {
      byte_offset += target_len;
    }
    if needle.is_empty() {
      return if byte_offset <= target_len {
        byte_offset
      } else {
        target_len
      };
    }
    return find_last_index(target, needle, byte_offset);
  }
  if needle.is_empty() {
    return if byte_offset <= target_len {
      byte_offset
    } else {
      target_len
    };
  }
  index_of_needle(
    target,
    needle,
    byte_offset,
    if encoding == 3 { 2 } else { 1 },
  )
}

fn index_of_buffer_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let encoding = args.get(3).int32_value(scope).unwrap_or(-1);
  if !(0..=7).contains(&encoding) {
    throw_error(scope, &format!("Unknown encoding code {encoding}"));
    return;
  }
  let target = match view_bytes(scope, args.get(0), "targetBuffer") {
    Some(target) => target,
    None => return,
  };
  let needle = match view_bytes(scope, args.get(1), "buffer") {
    Some(needle) => needle,
    None => return,
  };
  let byte_offset = args.get(2).integer_value(scope).unwrap_or(0);
  let forward = args.get(4).is_true();
  let index = index_of_buffer_from_bytes(
    &target,
    &needle,
    byte_offset,
    encoding,
    forward,
  );
  rv.set(v8::Number::new(scope, index as f64).into());
}

fn index_of_number_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let target = match view_bytes(scope, args.get(0), "targetBuffer") {
    Some(target) => target,
    None => return,
  };
  let number = args.get(1).uint32_value(scope).unwrap_or(0);
  let byte_offset = args.get(2).integer_value(scope).unwrap_or(0);
  let forward = args.get(3).is_true();
  let needle = [number as u8];
  let index =
    index_of_buffer_from_bytes(&target, &needle, byte_offset, 1, forward);
  rv.set(v8::Number::new(scope, index as f64).into());
}

fn index_of_needle_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let source = match view_bytes(scope, args.get(0), "source") {
    Some(source) => source,
    None => return,
  };
  let needle = match view_bytes(scope, args.get(1), "needle") {
    Some(needle) => needle,
    None => return,
  };
  let start = args.get(2).integer_value(scope).unwrap_or(0);
  let step = args.get(3).uint32_value(scope).unwrap_or(1).max(1) as usize;
  let index = index_of_needle(&source, &needle, start, step);
  rv.set(v8::Number::new(scope, index as f64).into());
}

fn fill_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let buffer_value = args.get(0);
  if !buffer_value.is_array_buffer_view() {
    throw_type_error(scope, "buffer must be an ArrayBufferView");
    return;
  }
  let buffer =
    v8::Local::<v8::ArrayBufferView>::try_from(buffer_value).unwrap();
  let len = buffer.byte_length();
  let mut start = args.get(2).integer_value(scope).unwrap_or(0);
  let mut end = args.get(3).integer_value(scope).unwrap_or(len as i64);
  if start < 0 {
    start = (len as i64 + start).max(0);
  }
  if end < 0 {
    end = (len as i64 + end).max(0);
  }
  let start = (start as usize).min(len);
  let end = (end as usize).min(len);
  if end <= start {
    rv.set(buffer_value);
    return;
  }

  let value = args.get(1);
  let pattern = if value.is_array_buffer_view() {
    view_bytes(scope, value, "value").unwrap_or_default()
  } else if value.is_string() {
    value
      .to_string(scope)
      .unwrap()
      .to_rust_string_lossy(scope)
      .into_bytes()
  } else {
    vec![value.uint32_value(scope).unwrap_or(0) as u8]
  };
  if pattern.is_empty() {
    rv.set(buffer_value);
    return;
  }

  // SAFETY: V8 gives a pointer to the ArrayBufferView data including its byte
  // offset. The write range is clamped to the same view's byte length.
  let data =
    unsafe { std::slice::from_raw_parts_mut(buffer.data().cast::<u8>(), len) };
  for (index, slot) in data[start..end].iter_mut().enumerate() {
    *slot = pattern[index % pattern.len()];
  }
  rv.set(buffer_value);
}

pub(crate) fn external_references() -> [ExternalReference; 4] {
  [
    INDEX_OF_BUFFER_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    INDEX_OF_NUMBER_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    INDEX_OF_NEEDLE_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    FILL_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
  ]
}

thread_local! {
  static INDEX_OF_BUFFER_CALLBACK: v8::FunctionCallback = index_of_buffer_callback.map_fn_to();
  static INDEX_OF_NUMBER_CALLBACK: v8::FunctionCallback = index_of_number_callback.map_fn_to();
  static INDEX_OF_NEEDLE_CALLBACK: v8::FunctionCallback = index_of_needle_callback.map_fn_to();
  static FILL_CALLBACK: v8::FunctionCallback = fill_callback.map_fn_to();
}

fn function_from_callback<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  callback: v8::FunctionCallback,
) -> v8::Local<'s, v8::Function> {
  v8::FunctionTemplate::new_raw(scope, callback)
    .get_function(scope)
    .unwrap()
}

#[op2]
pub fn op_node_internal_binding_buffer<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  let index_of_buffer = INDEX_OF_BUFFER_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "indexOfBuffer", index_of_buffer);
  let index_of_number = INDEX_OF_NUMBER_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "indexOfNumber", index_of_number);
  let fill =
    FILL_CALLBACK.with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "fill", fill);
  let index_of_needle = INDEX_OF_NEEDLE_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "indexOfNeedle", index_of_needle);

  let default_obj = v8::Object::new(scope);
  for name in ["indexOfBuffer", "indexOfNumber"] {
    let key = v8::String::new(scope, name).unwrap();
    let value = obj.get(scope, key.into()).unwrap();
    set_value(scope, default_obj, name, value);
  }
  set_value(scope, obj, "default", default_obj.into());
  obj
}
