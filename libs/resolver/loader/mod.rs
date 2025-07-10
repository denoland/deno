// Copyright 2018-2025 the Deno authors. MIT license.

mod npm;

#[cfg(all(feature = "graph", feature = "deno_ast"))]
mod prepared;

use std::borrow::Cow;

use deno_media_type::MediaType;
pub use npm::*;
#[cfg(all(feature = "graph", feature = "deno_ast"))]
pub use prepared::*;
use url::Url;

pub enum RequestedModuleType<'a> {
  None,
  Json,
  Text,
  Bytes,
  Other(&'a str),
}

#[allow(clippy::disallowed_types)]
type ArcStr = std::sync::Arc<str>;
#[allow(clippy::disallowed_types)]
type ArcBytes = std::sync::Arc<[u8]>;

pub struct LoadedModule<'a> {
  pub specifier: &'a Url,
  pub media_type: MediaType,
  pub source: LoadedModuleSource,
}

pub enum LoadedModuleSource {
  ArcStr(ArcStr),
  ArcBytes(ArcBytes),
  String(Cow<'static, str>),
  Bytes(Cow<'static, [u8]>),
}

impl LoadedModuleSource {
  pub fn as_bytes(&self) -> &[u8] {
    match self {
      LoadedModuleSource::ArcStr(text) => text.as_bytes(),
      LoadedModuleSource::ArcBytes(bytes) => bytes,
      LoadedModuleSource::String(text) => text.as_bytes(),
      LoadedModuleSource::Bytes(bytes) => bytes,
    }
  }
}
