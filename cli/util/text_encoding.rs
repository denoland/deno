// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use deno_core::ModuleCodeString;

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
