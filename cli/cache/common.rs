// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_runtime::deno_webstorage::rusqlite::Connection;

/// Very fast non-cryptographically secure hash.
pub fn fast_insecure_hash(bytes: &[u8]) -> u64 {
  use std::hash::Hasher;
  use twox_hash::XxHash64;

  let mut hasher = XxHash64::default();
  hasher.write(bytes);
  hasher.finish()
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
