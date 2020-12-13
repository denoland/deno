// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

mod async_cancel;
mod async_cell;
mod bindings;
pub mod build_util;
pub mod error;
mod flags;
mod gotham_state;
mod module_specifier;
mod modules;
mod normalize_path;
mod ops;
pub mod plugin_api;
mod resources;
mod resources2;
mod runtime;
mod shared_queue;
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
pub use crate::module_specifier::ModuleResolutionError;
pub use crate::module_specifier::ModuleSpecifier;
pub use crate::modules::FsModuleLoader;
pub use crate::modules::ModuleId;
pub use crate::modules::ModuleLoadId;
pub use crate::modules::ModuleLoader;
pub use crate::modules::ModuleSource;
pub use crate::modules::ModuleSourceFuture;
pub use crate::modules::NoopModuleLoader;
pub use crate::modules::RecursiveModuleLoad;
pub use crate::normalize_path::normalize_path;
pub use crate::ops::json_op_async;
pub use crate::ops::json_op_sync;
pub use crate::ops::op_close;
pub use crate::ops::op_resources;
pub use crate::ops::Op;
pub use crate::ops::OpAsyncFuture;
pub use crate::ops::OpFn;
pub use crate::ops::OpId;
pub use crate::ops::OpState;
pub use crate::ops::OpTable;
pub use crate::resources::ResourceTable;
pub use crate::resources2::Resource;
pub use crate::resources2::ResourceId;
pub use crate::resources2::ResourceTable2;
pub use crate::runtime::GetErrorClassFn;
pub use crate::runtime::JsErrorCreateFn;
pub use crate::runtime::JsRuntime;
pub use crate::runtime::RuntimeOptions;
pub use crate::runtime::Snapshot;
pub use crate::zero_copy_buf::BufVec;
pub use crate::zero_copy_buf::ZeroCopyBuf;

pub fn v8_version() -> &'static str {
  v8::V8::get_version()
}

#[test]
fn test_v8_version() {
  assert!(v8_version().len() > 3);
}
