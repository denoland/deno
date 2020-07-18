// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use std::io::{Error, ErrorKind};

/// Attempts to detect the character encoding of the provided bytes.
///
/// Supports UTF-8, UTF-16 Little Endian and UTF-16 Big Endian.
pub fn detect_charset(bytes: &[u8]) -> &str {
  const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";
  const UTF16_LE_BOM: &[u8] = b"\xFF\xFE";
  const UTF16_BE_BOM: &[u8] = b"\xFE\xFF";

  if bytes.starts_with(UTF8_BOM) {
    "utf-8"
  } else if bytes.starts_with(UTF16_LE_BOM) {
    "utf-16le"
  } else if bytes.starts_with(UTF16_BE_BOM) {
    "utf-16be"
  } else {
    // Assume everything else is utf-8
    "utf-8"
  }
}

/// Attempts to convert the provided bytes to a UTF-8 string.
///
/// Supports all encodings supported by the encoding crate, which includes all encodings specified in the WHATWG Encoding Standard (see: https://encoding.spec.whatwg.org/).
pub fn convert_to_utf8(bytes: &[u8], charset: &str) -> Result<String, Error> {
  match encoding::label::encoding_from_whatwg_label(charset) {
    Some(coder) => match coder.decode(bytes, encoding::DecoderTrap::Strict) {
      Ok(text) => Ok(text),
      Err(_e) => Err(Error::new(
        ErrorKind::Other,
        format!("Invalid data. Charset: {}", charset),
      )),
    },
    None => Err(Error::new(
      ErrorKind::Other,
      format!("Unsupported charset: {}", charset),
    )),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_detection(test_data: &[u8], expected_charset: &str) {
    let detected_charset = detect_charset(test_data);
    assert_eq!(
      expected_charset.to_lowercase(),
      detected_charset.to_lowercase()
    );
  }

  #[test]
  fn test_detection_utf8_no_bom() {
    let test_data = "Hello UTF-8 it is \u{23F0} for Deno!"
      .to_owned()
      .into_bytes();
    test_detection(&test_data, "utf-8");
  }

  #[test]
  fn test_detection_utf16_little_endian() {
    let test_data = b"\xFF\xFEHello UTF-16LE".to_owned().to_vec();
    test_detection(&test_data, "utf-16le");
  }

  #[test]
  fn test_detection_utf16_big_endian() {
    let test_data = b"\xFE\xFFHello UTF-16BE".to_owned().to_vec();
    test_detection(&test_data, "utf-16be");
  }

  #[test]
  fn test_decoding_unsupported_charset() {
    let test_data = Vec::new();
    let result = convert_to_utf8(&test_data, "utf-32le");
    assert!(result.is_err());
  }

  #[test]
  fn test_decoding_invalid_utf8() {
    let test_data = b"\xFE\xFE\xFF\xFF".to_vec();
    let result = convert_to_utf8(&test_data, "utf-8");
    assert!(result.is_err());
    let err = result.expect_err("Err expected");
    assert!(err.kind() == ErrorKind::InvalidData);
  }
}
