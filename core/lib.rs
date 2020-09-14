// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#[macro_use]
extern crate downcast_rs;
extern crate futures;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

mod bindings;
pub mod error;
mod flags;
mod gotham_state;
mod module_specifier;
mod modules;
mod normalize_path;
mod ops;
pub mod plugin_api;
mod resources;
mod runtime;
mod shared_queue;
mod zero_copy_buf;

pub use rusty_v8 as v8;

pub use crate::flags::v8_set_flags;
pub use crate::module_specifier::ModuleResolutionError;
pub use crate::module_specifier::ModuleSpecifier;
pub use crate::modules::ModuleId;
pub use crate::modules::ModuleLoadId;
pub use crate::modules::ModuleLoader;
pub use crate::modules::ModuleSource;
pub use crate::modules::ModuleSourceFuture;
pub use crate::modules::RecursiveModuleLoad;
pub use crate::normalize_path::normalize_path;
pub use crate::ops::json_op_async;
pub use crate::ops::json_op_sync;
pub use crate::ops::Op;
pub use crate::ops::OpAsyncFuture;
pub use crate::ops::OpFn;
pub use crate::ops::OpId;
pub use crate::ops::OpState;
pub use crate::ops::OpTable;
pub use crate::resources::ResourceTable;
pub use crate::runtime::js_check;
pub use crate::runtime::GetErrorClassFn;
pub use crate::runtime::HeapLimits;
pub use crate::runtime::JsRuntime;
pub use crate::runtime::JsRuntimeState;
pub use crate::runtime::RuntimeOptions;
pub use crate::runtime::Snapshot;
pub use crate::zero_copy_buf::BufVec;
pub use crate::zero_copy_buf::ZeroCopyBuf;
pub use serde_json;

pub fn v8_version() -> &'static str {
  v8::V8::get_version()
}

#[test]
fn test_v8_version() {
  assert!(v8_version().len() > 3);
}
