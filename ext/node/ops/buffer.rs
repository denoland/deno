// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::anyhow;
use deno_core::anyhow::Result;
use deno_core::op2;

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
) -> Result<Vec<u8>> {
  match (from_encoding, to_encoding) {
    ("utf8", "ascii") => Ok(utf8_to_ascii(source)),
    ("utf8", "latin1") => Ok(utf8_to_latin1(source)),
    ("utf8", "ucs2") => utf8_to_ucs2(source),
    ("ucs2", "utf8") | ("utf16le", "utf8") => ucs2_to_utf8(source),
    ("latin1", "ucs2")
    | ("latin1", "utf16le")
    | ("ascii", "ucs2")
    | ("ascii", "utf16le") => Ok(latin1_ascii_to_ucs2(source)),
    (from, to) => Err(anyhow!("Unable to transcode Buffer {from}->{to}")),
  }
}

fn latin1_ascii_to_ucs2(ascii: &[u8]) -> Vec<u8> {
  let mut result = Vec::with_capacity(ascii.len() * 2);
  for &byte in ascii {
    result.push(byte);
    result.push(0);
  }
  result
}

fn ucs2_to_utf8(ucs2_bytes: &[u8]) -> Result<Vec<u8>> {
  let ucs2_vec: Vec<u16> = ucs2_bytes
    .chunks(2)
    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
    .collect();
  String::from_utf16(&ucs2_vec)
    .map(|utf8_string| utf8_string.into_bytes())
    .map_err(|e| anyhow!("Invalid UTF-16 sequence: {}", e))
}

fn utf8_to_ucs2(utf8_bytes: &[u8]) -> Result<Vec<u8>> {
  let utf8_string = std::str::from_utf8(utf8_bytes)?;
  let ucs2_vec: Vec<u16> = utf8_string.encode_utf16().collect();
  let bytes: Vec<u8> = ucs2_vec.iter().flat_map(|&x| x.to_le_bytes()).collect();
  Ok(bytes)
}

fn utf8_to_latin1(utf8_bytes: &[u8]) -> Vec<u8> {
  let mut latin1_bytes = Vec::new();
  let mut i = 0;
  while i < utf8_bytes.len() {
    match utf8_bytes[i] {
      byte if byte <= 0x7F => {
        // ASCII character
        latin1_bytes.push(byte);
        i += 1;
      }
      byte if (0xC2..=0xDF).contains(&byte) && i + 1 < utf8_bytes.len() => {
        // 2-byte UTF-8 sequence
        let codepoint =
          ((byte as u16 & 0x1F) << 6) | (utf8_bytes[i + 1] as u16 & 0x3F);
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
        while i < utf8_bytes.len() && (utf8_bytes[i] & 0xC0) == 0x80 {
          i += 1;
        }
      }
    }
  }
  latin1_bytes
}

fn utf8_to_ascii(source: &[u8]) -> Vec<u8> {
  let mut ascii_bytes = Vec::new();
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
