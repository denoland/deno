// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::hash::Hasher;

use deno_core::error::AnyError;
use deno_runtime::deno_webstorage::rusqlite::Connection;

/// A very fast insecure hasher that uses the xxHash algorithm.
#[derive(Default)]
pub struct FastInsecureHasher(twox_hash::XxHash64);

impl FastInsecureHasher {
  pub fn new() -> Self {
    Self::default()
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
    hashable: &impl std::hash::Hash,
  ) -> &mut Self {
    hashable.hash(&mut self.0);
    self
  }

  pub fn finish(&self) -> u64 {
    self.0.finish()
  }
}

/// Runs the common sqlite pragma.
pub fn run_sqlite_pragma(conn: &Connection) -> Result<(), AnyError> {
  // Enable write-ahead-logging and tweak some other stuff
  let initial_pragmas = "
    -- enable write-ahead-logging mode
    PRAGMA journal_mode=WAL;
    PRAGMA synchronous=NORMAL;
    PRAGMA temp_store=memory;
    PRAGMA page_size=4096;
    PRAGMA mmap_size=6000000;
    PRAGMA optimize;
  ";

  conn.execute_batch(initial_pragmas)?;
  Ok(())
}
