// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_media_type::MediaType;
use deno_resolver::loader::LoadedModuleSource;
use deno_runtime::deno_core::FastString;
use deno_runtime::deno_core::ModuleSourceCode;
use deno_runtime::deno_core::ModuleType;
use deno_runtime::deno_core::RequestedModuleType;

pub fn module_type_from_media_and_requested_type(
  media_type: MediaType,
  requested_module_type: &RequestedModuleType,
) -> ModuleType {
  match requested_module_type {
    RequestedModuleType::Text => ModuleType::Text,
    RequestedModuleType::Bytes => ModuleType::Bytes,
    RequestedModuleType::None
    | RequestedModuleType::Other(_)
    | RequestedModuleType::Json => match media_type {
      MediaType::Json => ModuleType::Json,
      MediaType::Wasm => ModuleType::Wasm,
      _ => ModuleType::JavaScript,
    },
  }
}

pub fn loaded_module_source_to_module_source_code(
  loaded_module_source: LoadedModuleSource,
) -> ModuleSourceCode {
  match loaded_module_source {
    LoadedModuleSource::ArcStr(text) => ModuleSourceCode::String(text.into()),
    LoadedModuleSource::ArcBytes(bytes) => {
      ModuleSourceCode::Bytes(bytes.into())
    }
    LoadedModuleSource::String(text) => match text {
      Cow::Borrowed(static_text) => {
        ModuleSourceCode::String(FastString::from_static(static_text))
      }
      Cow::Owned(text) => ModuleSourceCode::String(text.into()),
    },
    LoadedModuleSource::Bytes(bytes) => match bytes {
      Cow::Borrowed(static_bytes) => {
        ModuleSourceCode::Bytes(static_bytes.into())
      }
      Cow::Owned(bytes) => {
        ModuleSourceCode::Bytes(bytes.into_boxed_slice().into())
      }
    },
  }
}

pub fn as_deno_resolver_requested_module_type(
  value: &RequestedModuleType,
) -> deno_resolver::loader::RequestedModuleType<'_> {
  match value {
    RequestedModuleType::None => {
      deno_resolver::loader::RequestedModuleType::None
    }
    RequestedModuleType::Json => {
      deno_resolver::loader::RequestedModuleType::Json
    }
    RequestedModuleType::Text => {
      deno_resolver::loader::RequestedModuleType::Text
    }
    RequestedModuleType::Bytes => {
      deno_resolver::loader::RequestedModuleType::Bytes
    }
    RequestedModuleType::Other(text) => {
      deno_resolver::loader::RequestedModuleType::Other(text)
    }
  }
}
