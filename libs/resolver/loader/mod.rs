// Copyright 2018-2025 the Deno authors. MIT license.

pub enum RequestedModuleType<'a> {
  None,
  Json,
  Text,
  Bytes,
  Other(&'a str),
}

#[cfg(all(feature = "graph", feature = "deno_ast"))]
mod prepared;

#[cfg(all(feature = "graph", feature = "deno_ast"))]
pub use prepared::*;
