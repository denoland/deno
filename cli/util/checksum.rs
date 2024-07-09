// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use ring::digest::Context;
use ring::digest::SHA256;

/// Generate a SHA256 checksum of a slice of byte-slice-like things.
pub fn gen(v: &[impl AsRef<[u8]>]) -> String {
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
    let actual = gen(&[b"hello world"]);
    assert_eq!(
      actual,
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
  }
}
