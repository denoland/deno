// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use deno_core::ModuleCodeString;

pub const BOM_CHAR: char = '\u{FEFF}';

/// Strips the byte order mark from the provided text if it exists.
pub fn strip_bom(text: &str) -> &str {
  if text.starts_with(BOM_CHAR) {
    &text[BOM_CHAR.len_utf8()..]
  } else {
    text
  }
}

static SOURCE_MAP_PREFIX: &[u8] =
  b"//# sourceMappingURL=data:application/json;base64,";

pub fn source_map_from_code(code: &ModuleCodeString) -> Option<Vec<u8>> {
  let bytes = code.as_bytes();
  let last_line = bytes.rsplit(|u| *u == b'\n').next()?;
  if last_line.starts_with(SOURCE_MAP_PREFIX) {
    let input = last_line.split_at(SOURCE_MAP_PREFIX.len()).1;
    let decoded_map = BASE64_STANDARD
      .decode(input)
      .expect("Unable to decode source map from emitted file.");
    Some(decoded_map)
  } else {
    None
  }
}

/// Truncate the source code before the source map.
pub fn code_without_source_map(mut code: ModuleCodeString) -> ModuleCodeString {
  let bytes = code.as_bytes();
  for i in (0..bytes.len()).rev() {
    if bytes[i] == b'\n' {
      if bytes[i + 1..].starts_with(SOURCE_MAP_PREFIX) {
        code.truncate(i + 1);
      }
      return code;
    }
  }
  code
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
    let err = result.expect_err("Err expected");
    assert!(err.kind() == ErrorKind::InvalidInput);
  }

  #[test]
  fn test_decoding_invalid_utf8() {
    let test_data = b"\xFE\xFE\xFF\xFF".to_vec();
    let result = convert_to_utf8(&test_data, "utf-8");
    assert!(result.is_err());
    let err = result.expect_err("Err expected");
    assert!(err.kind() == ErrorKind::InvalidData);
  }

  #[test]
  fn test_source_without_source_map() {
    run_test("", "");
    run_test("\n", "\n");
    run_test("\r\n", "\r\n");
    run_test("a", "a");
    run_test("a\n", "a\n");
    run_test("a\r\n", "a\r\n");
    run_test("a\r\nb", "a\r\nb");
    run_test("a\nb\n", "a\nb\n");
    run_test("a\r\nb\r\n", "a\r\nb\r\n");
    run_test(
      "test\n//# sourceMappingURL=data:application/json;base64,test",
      "test\n",
    );
    run_test(
      "test\r\n//# sourceMappingURL=data:application/json;base64,test",
      "test\r\n",
    );
    run_test(
      "\n//# sourceMappingURL=data:application/json;base64,test",
      "\n",
    );

    fn run_test(input: &'static str, output: &'static str) {
      assert_eq!(
        code_without_source_map(ModuleCodeString::from_static(input))
          .as_str()
          .to_owned(),
        output
      );
    }
  }
}
