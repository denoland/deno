// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::hash::Hasher;

/// A very fast insecure hasher that uses the xxHash algorithm.
#[derive(Default)]
pub struct FastInsecureHasher(twox_hash::XxHash64);

impl FastInsecureHasher {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn hash(hashable: impl std::hash::Hash) -> u64 {
    Self::new().write_hashable(hashable).finish()
  }

  pub fn write_str(&mut self, text: &str) -> &mut Self {
    self.write(text.as_bytes());
    self
  }

  pub fn write(&mut self, bytes: &[u8]) -> &mut Self {
    self.0.write(bytes);
    self
  }

  pub fn write_u8(&mut self, value: u8) -> &mut Self {
    self.0.write_u8(value);
    self
  }

  pub fn write_u64(&mut self, value: u64) -> &mut Self {
    self.0.write_u64(value);
    self
  }

  pub fn write_hashable(
    &mut self,
    hashable: impl std::hash::Hash,
  ) -> &mut Self {
    hashable.hash(&mut self.0);
    self
  }

  pub fn finish(&self) -> u64 {
    self.0.finish()
  }
}
