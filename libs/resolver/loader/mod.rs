// Copyright 2018-2025 the Deno authors. MIT license.

mod npm;

#[cfg(all(feature = "graph", feature = "deno_ast"))]
mod module_loader;

use std::borrow::Cow;

use deno_media_type::MediaType;
#[cfg(all(feature = "graph", feature = "deno_ast"))]
pub use module_loader::*;
pub use npm::*;
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

pub enum LoadedModuleOrAsset<'a> {
  Module(LoadedModule<'a>),
  /// A module that the graph knows about, but the data
  /// is not stored in the graph itself. It's up to the caller
  /// to fetch this data.
  ExternalAsset {
    statically_analyzable: bool,
    specifier: Cow<'a, Url>,
  },
}

pub struct LoadedModule<'a> {
  pub specifier: Cow<'a, Url>,
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
