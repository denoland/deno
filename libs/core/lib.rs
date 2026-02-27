// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]
#![deny(clippy::unused_async)]
#![deny(clippy::unnecessary_wraps)]

pub mod arena;
mod async_cancel;
mod async_cell;
pub mod convert;
pub mod cppgc;
pub mod error;
pub mod event_loop;
mod extension_set;
mod extensions;
mod external;
mod fast_string;
mod flags;
mod gotham_state;
mod inspector;
mod io;
mod module_specifier;
mod modules;
mod ops;
mod ops_builtin;
mod ops_builtin_types;
mod ops_builtin_v8;
mod ops_metrics;
pub mod reactor;
#[cfg(feature = "reactor-tokio")]
pub mod reactor_tokio;
mod runtime;
mod source_map;
mod tasks;
#[allow(
  non_camel_case_types,
  non_upper_case_globals,
  clippy::missing_safety_doc
)]
pub mod uv_compat;
mod web_timeout;
pub mod webidl;

// Re-exports
pub use anyhow;
pub use deno_ops::CppgcBase;
pub use deno_ops::CppgcInherits;
pub use deno_ops::FromV8;
pub use deno_ops::ToV8;
pub use deno_ops::WebIDL;
pub use deno_ops::op2;
pub use deno_unsync as unsync;
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
pub use thiserror;
pub use url;
pub use v8;

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
pub use crate::convert::FromV8;
pub use crate::convert::FromV8Scopeless;
pub use crate::convert::ToV8;
pub use crate::cppgc::GarbageCollected;
pub use crate::extensions::AccessorType;
pub use crate::extensions::Extension;
pub use crate::extensions::ExtensionArguments;
pub use crate::extensions::ExtensionFileSource;
pub use crate::extensions::ExtensionFileSourceCode;
pub use crate::extensions::Op;
pub use crate::extensions::OpDecl;
pub use crate::extensions::OpMiddlewareFn;
pub use crate::external::ExternalDefinition;
pub use crate::external::ExternalPointer;
pub use crate::external::Externalizable;
pub use crate::fast_string::FastStaticString;
pub use crate::fast_string::FastString;
pub use crate::fast_string::FastStringV8AllocationError;
pub use crate::flags::v8_set_flags;
pub use crate::inspector::InspectorMsg;
pub use crate::inspector::InspectorMsgKind;
pub use crate::inspector::InspectorSessionChannels;
pub use crate::inspector::InspectorSessionKind;
pub use crate::inspector::InspectorSessionProxy;
pub use crate::inspector::InspectorSessionSend;
pub use crate::inspector::JsRuntimeInspector;
pub use crate::inspector::LocalInspectorSession;
pub use crate::inspector::SessionContainer;
pub use crate::inspector::create_worker_inspector_session_pair;
pub use crate::io::AsyncResult;
pub use crate::io::BufMutView;
pub use crate::io::BufView;
pub use crate::io::Resource;
pub use crate::io::ResourceHandle;
pub use crate::io::ResourceHandleFd;
pub use crate::io::ResourceHandleSocket;
pub use crate::io::ResourceId;
pub use crate::io::ResourceTable;
pub use crate::io::TransferredResource;
pub use crate::io::WriteOutcome;
pub use crate::module_specifier::ModuleResolutionError;
pub use crate::module_specifier::ModuleSpecifier;
pub use crate::module_specifier::resolve_import;
pub use crate::module_specifier::resolve_url;
pub use crate::modules::CustomModuleEvaluationKind;
pub use crate::modules::ExtCodeCache;
pub use crate::modules::FsModuleLoader;
pub use crate::modules::ModuleCodeBytes;
pub use crate::modules::ModuleCodeString;
pub use crate::modules::ModuleId;
pub use crate::modules::ModuleLoadOptions;
pub use crate::modules::ModuleLoadReferrer;
pub use crate::modules::ModuleLoadResponse;
pub use crate::modules::ModuleLoader;
pub use crate::modules::ModuleName;
pub use crate::modules::ModuleSource;
pub use crate::modules::ModuleSourceCode;
pub use crate::modules::ModuleSourceFuture;
pub use crate::modules::ModuleType;
pub use crate::modules::NoopModuleLoader;
pub use crate::modules::RequestedModuleType;
pub use crate::modules::ResolutionKind;
pub use crate::modules::SourceCodeCacheInfo;
pub use crate::modules::StaticModuleLoader;
pub use crate::modules::ValidateImportAttributesCb;
pub use crate::ops::ExternalOpsTracker;
pub use crate::ops::OpId;
pub use crate::ops::OpMetadata;
pub use crate::ops::OpStackTraceCallback;
pub use crate::ops::OpState;
pub use crate::ops::PromiseId;
pub use crate::ops_builtin::op_close;
pub use crate::ops_builtin::op_print;
pub use crate::ops_builtin::op_resources;
pub use crate::ops_builtin::op_void_async;
pub use crate::ops_builtin::op_void_sync;
pub use crate::ops_metrics::OpMetricsEvent;
pub use crate::ops_metrics::OpMetricsFactoryFn;
pub use crate::ops_metrics::OpMetricsFn;
pub use crate::ops_metrics::OpMetricsSource;
pub use crate::ops_metrics::OpMetricsSummary;
pub use crate::ops_metrics::OpMetricsSummaryTracker;
pub use crate::ops_metrics::merge_op_metrics;
pub use crate::runtime::CONTEXT_STATE_SLOT_INDEX;
pub use crate::runtime::CompiledWasmModuleStore;
pub use crate::runtime::ContextState;
pub use crate::runtime::CreateRealmOptions;
pub use crate::runtime::CrossIsolateStore;
pub use crate::runtime::JsRuntime;
pub use crate::runtime::JsRuntimeForSnapshot;
pub use crate::runtime::MODULE_MAP_SLOT_INDEX;
pub use crate::runtime::PollEventLoopOptions;
pub use crate::runtime::RuntimeOptions;
pub use crate::runtime::SharedArrayBufferStore;
pub use crate::runtime::V8_WRAPPER_OBJECT_INDEX;
pub use crate::runtime::V8_WRAPPER_TYPE_INDEX;
pub use crate::runtime::stats;

pub use crate::source_map::SourceMapData;
pub use crate::tasks::V8CrossThreadTaskSpawner;
pub use crate::tasks::V8TaskSpawner;
pub use deno_path_util::normalize_path;
pub use deno_path_util::resolve_path;
pub use deno_path_util::resolve_url_or_path;
pub use deno_path_util::specifier_has_uri_scheme;

// Ensure we can use op2 in deno_core without any hackery.
extern crate self as deno_core;

/// An internal module re-exporting functions used by the #[op] (`deno_ops`) macro
#[doc(hidden)]
pub mod _ops {
  pub use super::cppgc::make_cppgc_object;
  pub use super::cppgc::make_cppgc_proto_object;
  pub use super::cppgc::try_unwrap_cppgc_base_object;
  pub use super::cppgc::try_unwrap_cppgc_base_persistent_object;
  pub use super::cppgc::try_unwrap_cppgc_object;
  pub use super::cppgc::try_unwrap_cppgc_persistent_object;
  pub use super::error::throw_error_js_error_class;
  pub use super::error::throw_error_one_byte;
  pub use super::error::throw_error_one_byte_info;
  pub use super::extensions::Op;
  pub use super::extensions::OpDecl;
  pub use super::extensions::OpMethodDecl;
  pub use super::ops::OpCtx;
  #[cfg(debug_assertions)]
  pub use super::ops::reentrancy_check;
  pub use super::ops_metrics::OpMetricsEvent;
  pub use super::ops_metrics::dispatch_metrics_async;
  pub use super::ops_metrics::dispatch_metrics_fast;
  pub use super::ops_metrics::dispatch_metrics_slow;
  pub use super::runtime::V8_WRAPPER_OBJECT_INDEX;
  pub use super::runtime::V8_WRAPPER_TYPE_INDEX;
  pub use super::runtime::ops::*;
  pub use super::runtime::ops_rust_to_v8::*;
  pub use inventory;
}

pub mod snapshot {
  pub use crate::runtime::CreateSnapshotOptions;
  pub use crate::runtime::CreateSnapshotOutput;
  pub use crate::runtime::FilterFn;
  pub use crate::runtime::create_snapshot;
  pub use crate::runtime::get_js_files;
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
      ::std::file!(),
      ":",
      ::std::line!(),
      ":",
      ::std::column!(),
      "]"
    )
  };
}

#[cfg(all(test, not(miri)))]
mod tests {
  use std::process::Command;
  use std::process::Stdio;

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

  // If the deno command is available, we ensure the async stubs are correctly rebuilt.
  #[test]
  #[cfg_attr(all(target_os = "windows", target_arch = "aarch64"), ignore)]
  fn test_rebuild_async_stubs() {
    // Check for deno first
    if let Err(e) = Command::new("deno")
      .arg("--version")
      .stderr(Stdio::null())
      .stdout(Stdio::null())
      .status()
    {
      #[allow(clippy::print_stderr)]
      {
        eprintln!("Ignoring test because we couldn't find deno: {e:?}");
      }
    }
    let status = Command::new("deno")
      .args(["run", "-A", "rebuild_async_stubs.js", "--check"])
      .stderr(Stdio::null())
      .stdout(Stdio::null())
      .status()
      .unwrap();
    assert!(
      status.success(),
      "Async stubs were not updated, or 'rebuild_async_stubs.js' failed for some other reason"
    );
  }
}
