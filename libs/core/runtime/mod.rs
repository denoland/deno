// Copyright 2018-2025 the Deno authors. MIT license.

pub(crate) mod bindings;
pub(crate) mod exception_state;
mod jsrealm;
mod jsruntime;
pub mod op_driver;
#[doc(hidden)]
pub mod ops;
pub mod ops_rust_to_v8;
mod setup;
mod snapshot;
pub mod stats;
pub mod v8_static_strings;

#[cfg(all(test, not(miri)))]
mod tests;

pub const V8_WRAPPER_TYPE_INDEX: i32 = 0;
pub const V8_WRAPPER_OBJECT_INDEX: i32 = 1;

pub use jsrealm::CONTEXT_STATE_SLOT_INDEX;
pub use jsrealm::ContextState;
pub(crate) use jsrealm::JsRealm;
pub use jsrealm::MODULE_MAP_SLOT_INDEX;
pub(crate) use jsrealm::OpDriverImpl;
pub(crate) use jsrealm::UnrefedOps;
pub use jsruntime::CompiledWasmModuleStore;
pub use jsruntime::CreateRealmOptions;
pub use jsruntime::CrossIsolateStore;
pub use jsruntime::ExtensionTranspiler;
pub(crate) use jsruntime::InitMode;
pub use jsruntime::JsRuntime;
pub use jsruntime::JsRuntimeForSnapshot;
pub use jsruntime::JsRuntimeState;
#[cfg(test)]
pub(crate) use jsruntime::NO_OF_BUILTIN_MODULES;
pub use jsruntime::PollEventLoopOptions;
pub use jsruntime::RuntimeOptions;
pub use jsruntime::SharedArrayBufferStore;
pub use snapshot::CreateSnapshotOptions;
pub use snapshot::CreateSnapshotOutput;
pub use snapshot::FilterFn;
pub(crate) use snapshot::SnapshotDataId;
pub(crate) use snapshot::SnapshotLoadDataStore;
pub(crate) use snapshot::SnapshotStoreDataStore;
pub(crate) use snapshot::SnapshottedData;
pub use snapshot::create_snapshot;
pub use snapshot::get_js_files;
