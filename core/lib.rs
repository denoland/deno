// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
mod async_cancel;
mod async_cell;
pub mod error;
mod error_codes;
mod extensions;
mod fast_string;
mod flags;
mod gotham_state;
mod inspector;
mod io;
mod module_specifier;
mod modules;
mod normalize_path;
mod ops;
mod ops_builtin;
mod ops_builtin_v8;
mod ops_metrics;
mod path;
mod resources;
mod runtime;
mod source_map;
pub mod task;
mod task_queue;

// Re-exports
pub use anyhow;
pub use futures;
pub use parking_lot;
pub use serde;
pub use serde_json;
pub use serde_v8;
pub use serde_v8::ByteString;
pub use serde_v8::DetachedBuffer;
pub use serde_v8::JsBuffer;
pub use serde_v8::StringOrBuffer;
pub use serde_v8::ToJsBuffer;
pub use serde_v8::U16String;
pub use sourcemap;
pub use url;
pub use v8;

pub use deno_ops::op;
pub use deno_ops::op2;

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
pub use crate::error::GetErrorClassFn;
pub use crate::error::JsErrorCreateFn;
pub use crate::extensions::Extension;
pub use crate::extensions::ExtensionBuilder;
pub use crate::extensions::ExtensionFileSource;
pub use crate::extensions::ExtensionFileSourceCode;
pub use crate::extensions::OpDecl;
pub use crate::extensions::OpMiddlewareFn;
pub use crate::fast_string::FastString;
pub use crate::flags::v8_set_flags;
pub use crate::inspector::InspectorMsg;
pub use crate::inspector::InspectorMsgKind;
pub use crate::inspector::InspectorSessionProxy;
pub use crate::inspector::JsRuntimeInspector;
pub use crate::inspector::LocalInspectorSession;
pub use crate::io::BufMutView;
pub use crate::io::BufView;
pub use crate::io::WriteOutcome;
pub use crate::module_specifier::resolve_import;
pub use crate::module_specifier::resolve_path;
pub use crate::module_specifier::resolve_url;
pub use crate::module_specifier::resolve_url_or_path;
pub use crate::module_specifier::ModuleResolutionError;
pub use crate::module_specifier::ModuleSpecifier;
pub use crate::modules::ExtModuleLoaderCb;
pub use crate::modules::FsModuleLoader;
pub use crate::modules::ModuleCode;
pub use crate::modules::ModuleId;
pub use crate::modules::ModuleLoader;
pub use crate::modules::ModuleSource;
pub use crate::modules::ModuleSourceFuture;
pub use crate::modules::ModuleType;
pub use crate::modules::NoopModuleLoader;
pub use crate::modules::ResolutionKind;
pub use crate::normalize_path::normalize_path;
pub use crate::ops::OpCall;
pub use crate::ops::OpError;
pub use crate::ops::OpId;
pub use crate::ops::OpResult;
pub use crate::ops::OpState;
pub use crate::ops::PromiseId;
pub use crate::ops_builtin::op_close;
pub use crate::ops_builtin::op_print;
pub use crate::ops_builtin::op_resources;
pub use crate::ops_builtin::op_void_async;
pub use crate::ops_builtin::op_void_sync;
pub use crate::ops_metrics::OpsTracker;
pub use crate::path::strip_unc_prefix;
pub use crate::resources::AsyncResult;
pub use crate::resources::Resource;
pub use crate::resources::ResourceId;
pub use crate::resources::ResourceTable;
pub use crate::runtime::CompiledWasmModuleStore;
pub use crate::runtime::CrossIsolateStore;
pub use crate::runtime::JsRealm;
pub use crate::runtime::JsRuntime;
pub use crate::runtime::JsRuntimeForSnapshot;
pub use crate::runtime::RuntimeOptions;
pub use crate::runtime::SharedArrayBufferStore;
pub use crate::runtime::Snapshot;
pub use crate::runtime::V8_WRAPPER_OBJECT_INDEX;
pub use crate::runtime::V8_WRAPPER_TYPE_INDEX;
pub use crate::source_map::SourceMapGetter;
pub use crate::task_queue::TaskQueue;
pub use crate::task_queue::TaskQueuePermit;

pub fn v8_version() -> &'static str {
  v8::V8::get_version()
}

/// An internal module re-exporting functions used by the #[op] (`deno_ops`) macro
#[doc(hidden)]
pub mod _ops {
  pub use super::error::throw_type_error;
  pub use super::error_codes::get_error_code;
  pub use super::extensions::OpDecl;
  pub use super::ops::to_op_result;
  pub use super::ops::OpCtx;
  pub use super::ops::OpResult;
  pub use super::runtime::ops::map_async_op1;
  pub use super::runtime::ops::map_async_op2;
  pub use super::runtime::ops::map_async_op3;
  pub use super::runtime::ops::map_async_op4;
  pub use super::runtime::ops::queue_async_op;
  pub use super::runtime::ops::queue_fast_async_op;
  pub use super::runtime::ops::to_i32;
  pub use super::runtime::ops::to_u32;
  pub use super::runtime::V8_WRAPPER_OBJECT_INDEX;
  pub use super::runtime::V8_WRAPPER_TYPE_INDEX;
}

// TODO(mmastrac): Temporary while we move code around
pub mod snapshot_util {
  pub use crate::runtime::create_snapshot;
  pub use crate::runtime::get_js_files;
  pub use crate::runtime::CreateSnapshotOptions;
  pub use crate::runtime::CreateSnapshotOutput;
  pub use crate::runtime::FilterFn;
}

/// A helper macro that will return a call site in Rust code. Should be
/// used when executing internal one-line scripts for JsRuntime lifecycle.
///
/// Returns a string in form of: "`[ext:<filename>:<line>:<column>]`"
#[macro_export]
macro_rules! located_script_name {
  () => {
    concat!(
      "[ext:",
      std::file!(),
      ":",
      std::line!(),
      ":",
      std::column!(),
      "]"
    )
  };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn located_script_name() {
    // Note that this test will fail if this file is moved. We don't
    // test line locations because that's just too brittle.
    let name = located_script_name!();
    let expected = if cfg!(windows) {
      "[ext:core\\lib.rs:"
    } else {
      "[ext:core/lib.rs:"
    };
    assert_eq!(&name[..expected.len()], expected);
  }

  #[test]
  fn test_v8_version() {
    assert!(v8_version().len() > 3);
  }
}
