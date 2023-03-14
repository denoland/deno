// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LockfileError {
  #[error(transparent)]
  Io(#[from] std::io::Error),

  #[error("Unable to read lockfile: \"{0}\"")]
  ReadError(String),

  #[error("Unable to parse contents of lockfile: \"{0}\"")]
  ParseError(String),
}
