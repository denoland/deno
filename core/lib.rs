// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
mod async_cancel;
mod async_cell;
mod bindings;
pub mod error;
mod flags;
mod gotham_state;
mod module_specifier;
mod modules;
mod normalize_path;
mod ops;
mod ops_bin;
mod ops_json;
pub mod plugin_api;
mod resources;
mod runtime;
mod zero_copy_buf;

// Re-exports
pub use futures;
pub use rusty_v8 as v8;
pub use serde;
pub use serde_json;
pub use url;

pub use crate::async_cancel::CancelFuture;
pub use crate::async_cancel::CancelHandle;
pub use crate::async_cancel::CancelTryFuture;
pub use crate::async_cancel::Cancelable;
pub use crate::async_cancel::Canceled;
pub use crate::async_cancel::TryCancelable;
pub use crate::async_cell::AsyncMut;
pub use crate::async_cell::AsyncMutFuture;
pub use crate::async_cell::AsyncRef;
pub use crate::async_cell::AsyncRefCell;
pub use crate::async_cell::AsyncRefFuture;
pub use crate::async_cell::RcLike;
pub use crate::async_cell::RcRef;
pub use crate::flags::v8_set_flags;
pub use crate::module_specifier::resolve_import;
pub use crate::module_specifier::resolve_path;
pub use crate::module_specifier::resolve_url;
pub use crate::module_specifier::resolve_url_or_path;
pub use crate::module_specifier::ModuleResolutionError;
pub use crate::module_specifier::ModuleSpecifier;
pub use crate::module_specifier::DUMMY_SPECIFIER;
pub use crate::modules::FsModuleLoader;
pub use crate::modules::ModuleId;
pub use crate::modules::ModuleLoadId;
pub use crate::modules::ModuleLoader;
pub use crate::modules::ModuleSource;
pub use crate::modules::ModuleSourceFuture;
pub use crate::modules::NoopModuleLoader;
pub use crate::modules::RecursiveModuleLoad;
pub use crate::normalize_path::normalize_path;
pub use crate::ops::op_close;
pub use crate::ops::op_resources;
pub use crate::ops::serialize_op_result;
pub use crate::ops::Op;
pub use crate::ops::OpAsyncFuture;
pub use crate::ops::OpFn;
pub use crate::ops::OpId;
pub use crate::ops::OpPayload;
pub use crate::ops::OpResponse;
pub use crate::ops::OpState;
pub use crate::ops::OpTable;
pub use crate::ops::PromiseId;
pub use crate::ops::Serializable;
pub use crate::ops_bin::bin_op_async;
pub use crate::ops_bin::bin_op_sync;
pub use crate::ops_bin::ValueOrVector;
pub use crate::ops_json::json_op_async;
pub use crate::ops_json::json_op_sync;
pub use crate::resources::Resource;
pub use crate::resources::ResourceId;
pub use crate::resources::ResourceTable;
pub use crate::runtime::GetErrorClassFn;
pub use crate::runtime::JsErrorCreateFn;
pub use crate::runtime::JsRuntime;
pub use crate::runtime::RuntimeOptions;
pub use crate::runtime::Snapshot;
pub use crate::zero_copy_buf::ZeroCopyBuf;

pub fn v8_version() -> &'static str {
  v8::V8::get_version()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_v8_version() {
    assert!(v8_version().len() > 3);
  }
}
