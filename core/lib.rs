// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#[macro_use]
extern crate downcast_rs;
extern crate futures;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

mod bindings;
mod core_isolate;
mod errors;
mod es_isolate;
mod flags;
mod module_specifier;
mod modules;
mod normalize_path;
mod ops;
pub mod plugin_api;
mod resources;
mod shared_queue;
mod zero_copy_buf;

pub use rusty_v8 as v8;

pub use crate::core_isolate::js_check;
pub use crate::core_isolate::CoreIsolate;
pub use crate::core_isolate::CoreIsolateState;
pub use crate::core_isolate::HeapLimits;
pub use crate::core_isolate::Script;
pub use crate::core_isolate::Snapshot;
pub use crate::core_isolate::StartupData;
pub use crate::errors::ErrBox;
pub use crate::errors::JSError;
pub use crate::es_isolate::EsIsolate;
pub use crate::es_isolate::EsIsolateState;
pub use crate::flags::v8_set_flags;
pub use crate::module_specifier::ModuleResolutionError;
pub use crate::module_specifier::ModuleSpecifier;
pub use crate::modules::Deps;
pub use crate::modules::ModuleId;
pub use crate::modules::ModuleLoadId;
pub use crate::modules::ModuleLoader;
pub use crate::modules::ModuleSource;
pub use crate::modules::ModuleSourceFuture;
pub use crate::modules::RecursiveModuleLoad;
pub use crate::normalize_path::normalize_path;
pub use crate::ops::Buf;
pub use crate::ops::Op;
pub use crate::ops::OpAsyncFuture;
pub use crate::ops::OpId;
pub use crate::resources::ResourceTable;
pub use crate::zero_copy_buf::ZeroCopyBuf;

pub fn v8_version() -> &'static str {
  v8::V8::get_version()
}

#[test]
fn test_v8_version() {
  assert!(v8_version().len() > 3);
}

crate_modules!();
