// Copyright 2018-2025 the Deno authors. MIT license.

use aws_lc_rs::digest::Context;
use aws_lc_rs::digest::SHA256;

/// Generate a SHA256 checksum of a slice of byte-slice-like things.
pub fn r#gen(v: &[impl AsRef<[u8]>]) -> String {
  let mut ctx = Context::new(&SHA256);
  for src in v {
    ctx.update(src.as_ref());
  }
  faster_hex::hex_string(ctx.finish().as_ref())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_gen() {
    let actual = r#gen(&[b"hello world"]);
    assert_eq!(
      actual,
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
  }
}
