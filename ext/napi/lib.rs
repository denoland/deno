// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(non_camel_case_types, reason = "matches Node-API naming conventions")]
#![allow(
  non_upper_case_globals,
  reason = "matches Node-API naming conventions"
)]
#![allow(
  clippy::undocumented_unsafe_blocks,
  reason = "pervasive FFI unsafe blocks throughout NAPI implementation"
)]
#![deny(clippy::missing_safety_doc)]

//! Symbols to be exported are now defined in this JSON file.
//! The `#[napi_sym]` macro checks for missing entries and panics.
//!
//! `./tools/napi/generate_symbols_list.js` is used to generate the LINK `cli/exports.def` on Windows,
//! which is also checked into git.
//!
//! To add a new napi function:
//! 1. Place `#[napi_sym]` on top of your implementation.
//! 2. Add the function's identifier to this JSON list.
//! 3. Finally, run `tools/napi/generate_symbols_list.js` to update `ext/napi/generated_symbol_exports_list_*.def`.

pub mod js_native_api;
pub mod node_api;
pub mod util;
pub mod uv;

use core::ptr::NonNull;
use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
pub use std::ffi::CStr;
pub use std::os::raw::c_char;
pub use std::os::raw::c_void;
use std::path::Path;
use std::path::PathBuf;
pub use std::ptr;
use std::rc::Rc;
#[cfg(unix)]
use std::sync::Arc;
use std::thread_local;

use deno_core::ExternalOpsTracker;
use deno_core::OpState;
use deno_core::V8CrossThreadTaskSpawner;
use deno_core::V8TaskSpawner;
use deno_core::op2;
use deno_core::parking_lot::RwLock;
use deno_core::url::Url;
// Expose common stuff for ease of use.
// `use deno_napi::*`
pub use deno_core::v8;
use deno_permissions::PermissionCheckError;
pub use denort_helper::DenoRtNativeAddonLoader;
pub use denort_helper::DenoRtNativeAddonLoaderRc;
#[cfg(unix)]
use libloading::os::unix::*;
#[cfg(windows)]
use libloading::os::windows::*;
pub use value::napi_value;

pub mod function;
// Only used to diagnose Windows addons that link against `node.exe`; on unix the
// helpers are exercised solely by their unit tests.
#[cfg_attr(
  unix,
  allow(dead_code, reason = "only used on Windows; unix runs the unit tests")
)]
mod pe;
mod value;

/// Render the source (underlying OS error) of a `libloading::Error` as a
/// `": <error>"` suffix, or an empty string when there is no source. libloading
/// does not include the OS error in its own `Display`.
fn fmt_lib_load_source(err: &libloading::Error) -> String {
  use std::error::Error;
  match err.source() {
    Some(source) => format!(": {source}"),
    None => String::new(),
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum NApiError {
  #[class(type)]
  #[error("Invalid path")]
  InvalidPath,
  #[class(type)]
  #[error(transparent)]
  DenoRtLoad(#[from] denort_helper::LoadError),
  #[class(type)]
  // libloading's `Display` for a failed load is opaque (e.g. just
  // `LoadLibraryExW failed` on Windows). Surface the underlying OS error via
  // `source` and the resolved path so the failure is actionable. See
  // denoland/deno#36622.
  #[error("{source}{}\n  path: {}", fmt_lib_load_source(source), path.display())]
  LibraryLoad {
    source: libloading::Error,
    path: PathBuf,
  },
  #[class(type)]
  #[error("Unable to find register Node-API module at {}", .0.display())]
  ModuleNotFound(PathBuf),
  #[class(type)]
  #[error(
    "Cannot load native addon at {}: it was built against the legacy Node.js \
     native addon API (NODE_MODULE / nan), which Deno does not support. Only \
     Node-API (N-API) addons are supported.",
    .0.display()
  )]
  UnsupportedLegacyAddon(PathBuf),
  #[class(type)]
  #[error(
    "Cannot load native addon at {}: it links directly against the Node.js \
     binary (node.exe) and relies on the V8 C++ ABI, Node.js internals and/or \
     libuv exported by that executable, none of which Deno provides. An addon \
     can use Node-API (N-API) and still be unsupported when it has a regular \
     import of node.exe. The addon must be rebuilt to avoid linking directly \
     against node.exe, for example by using delay-loaded node.exe imports.",
    .0.display()
  )]
  UnsupportedNodeBinaryAddon(PathBuf),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] PermissionCheckError),
}

pub type napi_status = i32;
pub type napi_env = *mut c_void;
pub type napi_callback_info = *mut c_void;
pub type napi_deferred = *mut c_void;
pub type napi_ref = *mut c_void;
pub type napi_threadsafe_function = *mut c_void;
pub type napi_handle_scope = *mut c_void;
pub type napi_callback_scope = *mut c_void;
pub type napi_escapable_handle_scope = *mut c_void;
pub type napi_async_cleanup_hook_handle = *mut c_void;
pub type napi_async_work = *mut c_void;
pub type napi_async_context = *mut c_void;

pub const napi_ok: napi_status = 0;
pub const napi_invalid_arg: napi_status = 1;
pub const napi_object_expected: napi_status = 2;
pub const napi_string_expected: napi_status = 3;
pub const napi_name_expected: napi_status = 4;
pub const napi_function_expected: napi_status = 5;
pub const napi_number_expected: napi_status = 6;
pub const napi_boolean_expected: napi_status = 7;
pub const napi_array_expected: napi_status = 8;
pub const napi_generic_failure: napi_status = 9;
pub const napi_pending_exception: napi_status = 10;
pub const napi_cancelled: napi_status = 11;
pub const napi_escape_called_twice: napi_status = 12;
pub const napi_handle_scope_mismatch: napi_status = 13;
pub const napi_callback_scope_mismatch: napi_status = 14;
pub const napi_queue_full: napi_status = 15;
pub const napi_closing: napi_status = 16;
pub const napi_bigint_expected: napi_status = 17;
pub const napi_date_expected: napi_status = 18;
pub const napi_arraybuffer_expected: napi_status = 19;
pub const napi_detachable_arraybuffer_expected: napi_status = 20;
pub const napi_would_deadlock: napi_status = 21;
pub const napi_no_external_buffers_allowed: napi_status = 22;
pub const napi_cannot_run_js: napi_status = 23;

pub static ERROR_MESSAGES: &[&CStr] = &[
  c"",
  c"Invalid argument",
  c"An object was expected",
  c"A string was expected",
  c"A string or symbol was expected",
  c"A function was expected",
  c"A number was expected",
  c"A boolean was expected",
  c"An array was expected",
  c"Unknown failure",
  c"An exception is pending",
  c"The async work item was cancelled",
  c"napi_escape_handle already called on scope",
  c"Invalid handle scope usage",
  c"Invalid callback scope usage",
  c"Thread-safe function queue is full",
  c"Thread-safe function handle is closing",
  c"A bigint was expected",
  c"A date was expected",
  c"An arraybuffer was expected",
  c"A detachable arraybuffer was expected",
  c"Main thread would deadlock",
  c"External buffers are not allowed",
  c"Cannot run JavaScript",
];

pub const NAPI_AUTO_LENGTH: usize = usize::MAX;

/// `nm_version` value used by Node-API (N-API) modules. Legacy V8/nan addons
/// registered via `node_module_register` use `NODE_MODULE_VERSION` instead.
pub const NAPI_MODULE_VERSION: i32 = 1;

thread_local! {
  pub static MODULE_TO_REGISTER: RefCell<Option<*const NapiModule>> = const { RefCell::new(None) };
}

type napi_addon_register_func =
  unsafe extern "C" fn(env: napi_env, exports: napi_value) -> napi_value;
type napi_register_module_v1 =
  unsafe extern "C" fn(env: napi_env, exports: napi_value) -> napi_value;

#[repr(C)]
#[derive(Clone)]
pub struct NapiModule {
  pub nm_version: i32,
  pub nm_flags: u32,
  nm_filename: *const c_char,
  pub nm_register_func: napi_addon_register_func,
  nm_modname: *const c_char,
  nm_priv: *mut c_void,
  reserved: [*mut c_void; 4],
}

pub type napi_valuetype = i32;

pub const napi_undefined: napi_valuetype = 0;
pub const napi_null: napi_valuetype = 1;
pub const napi_boolean: napi_valuetype = 2;
pub const napi_number: napi_valuetype = 3;
pub const napi_string: napi_valuetype = 4;
pub const napi_symbol: napi_valuetype = 5;
pub const napi_object: napi_valuetype = 6;
pub const napi_function: napi_valuetype = 7;
pub const napi_external: napi_valuetype = 8;
pub const napi_bigint: napi_valuetype = 9;

pub type napi_threadsafe_function_release_mode = i32;

pub const napi_tsfn_release: napi_threadsafe_function_release_mode = 0;
pub const napi_tsfn_abort: napi_threadsafe_function_release_mode = 1;

pub type napi_threadsafe_function_call_mode = i32;

pub const napi_tsfn_nonblocking: napi_threadsafe_function_call_mode = 0;
pub const napi_tsfn_blocking: napi_threadsafe_function_call_mode = 1;

pub type napi_key_collection_mode = i32;

pub const napi_key_include_prototypes: napi_key_collection_mode = 0;
pub const napi_key_own_only: napi_key_collection_mode = 1;

pub type napi_key_filter = i32;

pub const napi_key_all_properties: napi_key_filter = 0;
pub const napi_key_writable: napi_key_filter = 1;
pub const napi_key_enumerable: napi_key_filter = 1 << 1;
pub const napi_key_configurable: napi_key_filter = 1 << 2;
pub const napi_key_skip_strings: napi_key_filter = 1 << 3;
pub const napi_key_skip_symbols: napi_key_filter = 1 << 4;

pub type napi_key_conversion = i32;

pub const napi_key_keep_numbers: napi_key_conversion = 0;
pub const napi_key_numbers_to_strings: napi_key_conversion = 1;

pub type napi_typedarray_type = i32;

pub const napi_int8_array: napi_typedarray_type = 0;
pub const napi_uint8_array: napi_typedarray_type = 1;
pub const napi_uint8_clamped_array: napi_typedarray_type = 2;
pub const napi_int16_array: napi_typedarray_type = 3;
pub const napi_uint16_array: napi_typedarray_type = 4;
pub const napi_int32_array: napi_typedarray_type = 5;
pub const napi_uint32_array: napi_typedarray_type = 6;
pub const napi_float32_array: napi_typedarray_type = 7;
pub const napi_float64_array: napi_typedarray_type = 8;
pub const napi_bigint64_array: napi_typedarray_type = 9;
pub const napi_biguint64_array: napi_typedarray_type = 10;
pub const napi_float16_array: napi_typedarray_type = 11;

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct napi_type_tag {
  pub lower: u64,
  pub upper: u64,
}

pub type napi_callback = unsafe extern "C" fn(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value<'static>;

pub type napi_finalize = unsafe extern "C" fn(
  env: napi_env,
  data: *mut c_void,
  finalize_hint: *mut c_void,
);

pub type napi_async_execute_callback =
  unsafe extern "C" fn(env: napi_env, data: *mut c_void);

pub type napi_async_complete_callback =
  unsafe extern "C" fn(env: napi_env, status: napi_status, data: *mut c_void);

pub type napi_threadsafe_function_call_js = unsafe extern "C" fn(
  env: napi_env,
  js_callback: napi_value,
  context: *mut c_void,
  data: *mut c_void,
);

pub type napi_async_cleanup_hook = unsafe extern "C" fn(
  handle: napi_async_cleanup_hook_handle,
  data: *mut c_void,
);

pub type napi_cleanup_hook = unsafe extern "C" fn(data: *mut c_void);

pub type napi_property_attributes = i32;

pub const napi_default: napi_property_attributes = 0;
pub const napi_writable: napi_property_attributes = 1 << 0;
pub const napi_enumerable: napi_property_attributes = 1 << 1;
pub const napi_configurable: napi_property_attributes = 1 << 2;
pub const napi_static: napi_property_attributes = 1 << 10;
pub const napi_default_method: napi_property_attributes =
  napi_writable | napi_configurable;
pub const napi_default_jsproperty: napi_property_attributes =
  napi_enumerable | napi_configurable | napi_writable;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct napi_property_descriptor<'a> {
  pub utf8name: *const c_char,
  pub name: napi_value<'a>,
  pub method: Option<napi_callback>,
  pub getter: Option<napi_callback>,
  pub setter: Option<napi_callback>,
  pub value: napi_value<'a>,
  pub attributes: napi_property_attributes,
  pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug)]
pub struct napi_extended_error_info {
  pub error_message: *const c_char,
  pub engine_reserved: *mut c_void,
  pub engine_error_code: i32,
  pub error_code: Cell<napi_status>,
}

#[repr(C)]
#[derive(Debug)]
pub struct napi_node_version {
  pub major: u32,
  pub minor: u32,
  pub patch: u32,
  pub release: *const c_char,
}

pub trait PendingNapiAsyncWork: FnOnce() + Send + 'static {}
impl<T> PendingNapiAsyncWork for T where T: FnOnce() + Send + 'static {}

/// A pending NAPI finalizer callback to be called at environment shutdown.
/// This matches Node.js's behavior of tracking references with finalize
/// callbacks and calling them during `napi_env::DeleteMe()`.
pub struct PendingNapiFinalizer {
  /// Unique identity of this registration. Finalizers must be deregistered by
  /// id, never by `data`: several live registrations can share the same `data`
  /// pointer (addons routinely pass a null or repeated `native_object`), and
  /// removing an arbitrary entry with a matching `data` leaves the entry whose
  /// callback already ran behind, which then runs a second time at shutdown.
  ///
  /// `None` for entries in [`RefTracker::gc_ready`]: those were already
  /// deregistered by id when the GC handed them over, and are only waiting for
  /// a JS-safe point to run, so nothing can look them up again.
  pub id: Option<NapiFinalizerId>,
  pub env: napi_env,
  pub cb: napi_finalize,
  pub data: *mut c_void,
  pub hint: *mut c_void,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NapiFinalizerId(u64);

/// Tracked finalizer callbacks that should be called at shutdown.
/// Matches Node.js `finalizing_reflist` / `reflist` behavior.
#[derive(Default)]
pub struct RefTracker {
  next_id: u64,
  pending: Vec<PendingNapiFinalizer>,
  /// Finalizers whose objects have already been collected by the GC and are
  /// waiting to run at the next point where JavaScript execution is legal
  /// (see [`Env::defer_gc_finalizer`]). Kept separate from `pending` because
  /// these are already committed to run exactly once — they must not be
  /// confused with finalizers for objects that are still alive.
  gc_ready: Vec<PendingNapiFinalizer>,
  /// Whether an event-loop task has already been scheduled to drain
  /// `gc_ready`. Avoids scheduling one task per collected object when a single
  /// GC cycle reclaims many references at once.
  drain_requested: bool,
}

impl RefTracker {
  /// Removes and returns all finalizers still owed at environment teardown:
  /// GC-collected finalizers that never got a chance to drain, plus
  /// finalizers for objects that were never collected.
  pub fn take_pending(&mut self) -> Vec<PendingNapiFinalizer> {
    self.drain_requested = false;
    let mut all = std::mem::take(&mut self.gc_ready);
    all.append(&mut self.pending);
    all
  }

  /// Queues a GC-collected finalizer to run at the next JS-safe point,
  /// returning `true` if the caller should schedule a fresh drain task (i.e.
  /// no drain was already pending).
  fn defer(&mut self, finalizer: PendingNapiFinalizer) -> bool {
    self.gc_ready.push(finalizer);
    let need_schedule = !self.drain_requested;
    self.drain_requested = true;
    need_schedule
  }

  /// Removes and returns the GC-collected finalizers ready to run, clearing
  /// the "drain requested" flag so that finalizers queued from here on
  /// schedule a new drain task.
  fn take_gc_ready(&mut self) -> Vec<PendingNapiFinalizer> {
    self.drain_requested = false;
    std::mem::take(&mut self.gc_ready)
  }

  fn add(
    &mut self,
    env: napi_env,
    cb: napi_finalize,
    data: *mut c_void,
    hint: *mut c_void,
  ) -> NapiFinalizerId {
    let id = NapiFinalizerId(self.next_id);
    self.next_id += 1;
    self.pending.push(PendingNapiFinalizer {
      id: Some(id),
      env,
      cb,
      data,
      hint,
    });
    id
  }

  /// Removes the entry with `id`, returning `true` if it was still pending.
  /// The return value lets the GC weak callback and env teardown coordinate
  /// the "run once" contract: whichever path removes the entry first runs the
  /// finalizer, and the other observes `false` and skips it.
  #[must_use = "the return value decides whether the finalizer may still \
                be run"]
  fn remove(&mut self, id: NapiFinalizerId) -> bool {
    if let Some(pos) = self.pending.iter().rposition(|f| f.id == Some(id)) {
      self.pending.remove(pos);
      true
    } else {
      false
    }
  }
}

pub struct NapiState {
  // Thread safe functions.
  pub env_cleanup_hooks: Rc<RefCell<Vec<(napi_cleanup_hook, *mut c_void)>>>,
  pub ref_tracker: Rc<RefCell<RefTracker>>,
  pub env_shared_ptrs: Vec<*mut EnvShared>,
  /// Raw Env pointers for teardown of external string finalizers.
  pub env_ptrs: Vec<*mut Env>,
  /// Per-isolate V8 Private key for napi_wrap/napi_unwrap.
  /// Node.js stores this per-isolate so that objects wrapped by one addon
  /// can be unwrapped by another. Lazily initialized on first addon load.
  pub napi_wrap: Option<v8::Global<v8::Private>>,
  /// Per-isolate V8 Private key for type tags.
  pub type_tag: Option<v8::Global<v8::Private>>,
}

// SAFETY: finalizer pointers in env_shared_ptrs are only accessed during Drop
// on the same thread that created them.
unsafe impl Send for NapiState {}

impl Drop for NapiState {
  fn drop(&mut self) {
    // External string resources can outlive their Env until V8 disposes the
    // isolate. Stop exposing each Env to those eventual callbacks. Callbacks
    // whose V8 resource was already disposed are completed here with their
    // original Env; the rest keep their registry entry, which the isolate's
    // disposal reclaims by firing the V8 destructor with a null Env. They
    // cannot be swept here because V8 still reads their buffers.
    for env_ptr in &self.env_ptrs {
      crate::js_native_api::detach_external_string_env(*env_ptr);
    }

    // Invalidate each Env's poll scope before addon cleanup hooks run, so
    // queued readiness cannot invoke addon callbacks during cleanup. The
    // worker identifies handles and scopes only by numeric token and scope ID.
    #[cfg(unix)]
    for env_ptr in &self.env_ptrs {
      unsafe { (*(*env_ptr)).poll_scope.invalidate() };
    }

    let hooks = {
      let h = self.env_cleanup_hooks.borrow_mut();
      h.clone()
    };

    // Hooks are supposed to be run in LIFO order
    let hooks_to_run = hooks.into_iter().rev();

    for hook in hooks_to_run {
      // This hook might have been removed by a previous hook, in such case skip it here.
      if !self
        .env_cleanup_hooks
        .borrow()
        .iter()
        .any(|pair| std::ptr::fn_addr_eq(pair.0, hook.0) && pair.1 == hook.1)
      {
        continue;
      }

      unsafe {
        (hook.0)(hook.1);
      }

      {
        self.env_cleanup_hooks.borrow_mut().retain(|pair| {
          !(std::ptr::fn_addr_eq(pair.0, hook.0) && pair.1 == hook.1)
        });
      }
    }

    // Note: remaining ref tracker entries are not finalized here because
    // V8 is no longer alive. MainWorker::run_napi_ref_finalizers() handles
    // this by calling finalizers directly while V8 is still alive.

    // Call instance data finalize callbacks for all registered EnvShared instances.
    // Each entry should be unique since each op_napi_open creates a fresh EnvShared.
    debug_assert!(
      {
        let mut seen = std::collections::HashSet::new();
        self.env_shared_ptrs.iter().all(|p| seen.insert(*p))
      },
      "env_shared_ptrs contains duplicate entries"
    );
    for env_shared_ptr in &self.env_shared_ptrs {
      // SAFETY: env_shared_ptr was created via Box::into_raw in op_napi_open
      // and the native module library is kept alive (via std::mem::forget).
      let env_shared = unsafe { &mut **env_shared_ptr };
      if let Some(instance_data) = env_shared.instance_data.take()
        && let Some(cb) = instance_data.finalize_cb
      {
        unsafe {
          cb(
            std::ptr::null_mut(),
            instance_data.data,
            instance_data.finalize_hint,
          );
        }
      }
    }
  }
}

#[repr(C)]
#[derive(Debug)]
pub struct InstanceData {
  pub data: *mut c_void,
  pub finalize_cb: Option<napi_finalize>,
  pub finalize_hint: *mut c_void,
}

#[repr(C)]
#[derive(Debug)]
/// Env that is shared between all contexts in same native module.
pub struct EnvShared {
  pub instance_data: Option<InstanceData>,
  pub napi_wrap: v8::Global<v8::Private>,
  pub type_tag: v8::Global<v8::Private>,
  pub finalize: Option<napi_finalize>,
  pub finalize_hint: *mut c_void,
  pub filename: String,
}

impl EnvShared {
  pub fn new(
    napi_wrap: v8::Global<v8::Private>,
    type_tag: v8::Global<v8::Private>,
    filename: String,
  ) -> Self {
    Self {
      instance_data: None,
      napi_wrap,
      type_tag,
      finalize: None,
      finalize_hint: std::ptr::null_mut(),
      filename,
    }
  }
}

#[repr(C)]
pub struct Env {
  context: NonNull<v8::Context>,
  pub isolate_ptr: v8::UnsafeRawIsolatePtr,
  pub open_handle_scopes: usize,
  pub open_callback_scopes: usize,
  pub shared: *mut EnvShared,
  pub async_work_sender: V8CrossThreadTaskSpawner,
  /// Same-thread event-loop task queue, used to run GC-collected finalizers at
  /// the next JS-safe point (see [`Env::defer_gc_finalizer`]). Only ever
  /// touched on the isolate thread; like the `Rc` fields below it relies on the
  /// `unsafe impl Send for Env` never actually exercising these off-thread.
  gc_finalizer_spawner: V8TaskSpawner,
  cleanup_hooks: Rc<RefCell<Vec<(napi_cleanup_hook, *mut c_void)>>>,
  ref_tracker: Rc<RefCell<RefTracker>>,
  external_ops_tracker: ExternalOpsTracker,
  pub last_error: napi_extended_error_info,
  pub last_exception: Option<v8::Global<v8::Value>>,
  pub global: v8::Global<v8::Object>,
  pub create_buffer: v8::Global<v8::Function>,
  pub report_error: v8::Global<v8::Function>,
  pub async_hooks_init: v8::Global<v8::Function>,
  pub async_hooks_before: v8::Global<v8::Function>,
  pub async_hooks_after: v8::Global<v8::Function>,
  pub async_hooks_destroy: v8::Global<v8::Function>,
  pub next_async_id: i64,
  #[cfg(unix)]
  pub(crate) uv_loop: *mut deno_core::uv_compat::uv_loop_t,
  #[cfg(unix)]
  pub(crate) uv_loop_liveness: Arc<deno_core::uv_compat::UvLoopLiveness>,
  #[cfg(unix)]
  pub(crate) poll_scope: deno_core::uv_compat::UvPollScope,
}

unsafe impl Send for Env {}
unsafe impl Sync for Env {}

impl Env {
  #[allow(clippy::too_many_arguments, reason = "construction")]
  pub fn new(
    isolate_ptr: v8::UnsafeRawIsolatePtr,
    context: v8::Global<v8::Context>,
    global: v8::Global<v8::Object>,
    create_buffer: v8::Global<v8::Function>,
    report_error: v8::Global<v8::Function>,
    async_hooks_init: v8::Global<v8::Function>,
    async_hooks_before: v8::Global<v8::Function>,
    async_hooks_after: v8::Global<v8::Function>,
    async_hooks_destroy: v8::Global<v8::Function>,
    sender: V8CrossThreadTaskSpawner,
    gc_finalizer_spawner: V8TaskSpawner,
    cleanup_hooks: Rc<RefCell<Vec<(napi_cleanup_hook, *mut c_void)>>>,
    ref_tracker: Rc<RefCell<RefTracker>>,
    external_ops_tracker: ExternalOpsTracker,
    #[cfg(unix)] uv_loop: *mut deno_core::uv_compat::uv_loop_t,
    #[cfg(unix)] uv_loop_liveness: Arc<deno_core::uv_compat::UvLoopLiveness>,
    #[cfg(unix)] poll_scope: deno_core::uv_compat::UvPollScope,
  ) -> Self {
    Self {
      isolate_ptr,
      context: context.into_raw(),
      global,
      create_buffer,
      report_error,
      async_hooks_init,
      async_hooks_before,
      async_hooks_after,
      async_hooks_destroy,
      next_async_id: 1,
      #[cfg(unix)]
      uv_loop,
      #[cfg(unix)]
      uv_loop_liveness,
      #[cfg(unix)]
      poll_scope,
      shared: std::ptr::null_mut(),
      open_handle_scopes: 0,
      open_callback_scopes: 0,
      async_work_sender: sender,
      gc_finalizer_spawner,
      cleanup_hooks,
      ref_tracker,
      external_ops_tracker,
      last_error: napi_extended_error_info {
        error_message: std::ptr::null(),
        engine_reserved: std::ptr::null_mut(),
        engine_error_code: 0,
        error_code: Cell::new(napi_ok),
      },
      last_exception: None,
    }
  }

  pub fn shared(&self) -> &EnvShared {
    // SAFETY: the lifetime of `EnvShared` always exceeds the lifetime of `Env`.
    unsafe { &*self.shared }
  }

  pub fn shared_mut(&mut self) -> &mut EnvShared {
    // SAFETY: the lifetime of `EnvShared` always exceeds the lifetime of `Env`.
    unsafe { &mut *self.shared }
  }

  pub fn add_async_work(&mut self, async_work: impl FnOnce() + Send + 'static) {
    self.async_work_sender.spawn(|_| async_work());
  }

  #[inline]
  pub fn isolate(&mut self) -> &mut v8::Isolate {
    // SAFETY: Lifetime of `Isolate` is longer than `Env`.
    unsafe {
      v8::Isolate::ref_from_raw_isolate_ptr_mut_unchecked(&mut self.isolate_ptr)
    }
  }

  pub fn context<'s>(&'s self) -> v8::Local<'s, v8::Context> {
    // SAFETY: `v8::Local` is always non-null pointer; the `PinScope<'_, '_>` is
    // already on the stack, but we don't have access to it.
    unsafe {
      std::mem::transmute::<NonNull<v8::Context>, v8::Local<v8::Context>>(
        self.context,
      )
    }
  }

  pub fn threadsafe_function_ref(&mut self) {
    self.external_ops_tracker.ref_op();
  }

  pub fn threadsafe_function_unref(&mut self) {
    self.external_ops_tracker.unref_op();
  }

  pub fn add_cleanup_hook(
    &mut self,
    hook: napi_cleanup_hook,
    data: *mut c_void,
  ) {
    let mut hooks = self.cleanup_hooks.borrow_mut();
    if hooks
      .iter()
      .any(|pair| std::ptr::fn_addr_eq(pair.0, hook) && pair.1 == data)
    {
      panic!("Cannot register cleanup hook with same data twice");
    }
    hooks.push((hook, data));
  }

  pub fn remove_cleanup_hook(
    &mut self,
    hook: napi_cleanup_hook,
    data: *mut c_void,
  ) {
    let mut hooks = self.cleanup_hooks.borrow_mut();
    match hooks
      .iter()
      .rposition(|&pair| std::ptr::fn_addr_eq(pair.0, hook) && pair.1 == data)
    {
      Some(index) => {
        hooks.remove(index);
      }
      None => panic!("Cannot remove cleanup hook which was not registered"),
    }
  }

  pub fn add_ref_finalizer(
    &self,
    env: napi_env,
    cb: napi_finalize,
    data: *mut c_void,
    hint: *mut c_void,
  ) -> NapiFinalizerId {
    self.ref_tracker.borrow_mut().add(env, cb, data, hint)
  }

  /// Deregisters a shutdown finalizer entry, returning `true` if it was still
  /// pending (see [`RefTracker::remove`]).
  #[must_use = "the return value decides whether the finalizer may still \
                be run"]
  pub fn remove_ref_finalizer(&self, id: NapiFinalizerId) -> bool {
    self.ref_tracker.borrow_mut().remove(id)
  }

  /// Queues a GC-collected reference's finalizer to run at the next JS-safe
  /// point on the event loop, the way Node drains finalizers from a
  /// `SetImmediate` rather than from the GC.
  ///
  /// napi finalizers are allowed to call back into JavaScript (e.g.
  /// `napi_call_function`), but V8's second-pass weak callback — where the GC
  /// hands us collected references — runs inside a
  /// `DisallowJavascriptExecutionScope`, so calling into JS there aborts the
  /// process (#36568). A V8 `RequestInterrupt` is not enough either: it is
  /// serviced from the stack guard, which V8 also checks while unwinding a GC,
  /// so the drain can still land inside the disallowed scope. The same-thread
  /// [`V8TaskSpawner`] instead runs the drain from `dispatch_task_spawner`
  /// during an event-loop poll, with a real context scope and a microtask
  /// checkpoint — a genuinely JS-safe point.
  ///
  /// This is *not* the cross-thread spawner that #33260 used and #34023
  /// reverted: that hazard was a finalizer running twice, which the
  /// `reset()` / `was_pending` run-once handshake (#36499) now prevents
  /// independently. The task only pushes onto an isolate-local queue on the
  /// isolate thread.
  ///
  /// Takes the `Env` as a raw pointer rather than `&mut self` on purpose: the
  /// caller (the GC weak callback) already holds a `*mut Env`, and the queued
  /// finalizer will mutate the `Env` through it long after this call returns.
  /// Round-tripping that pointer through a reference here would hand the
  /// finalizer a pointer derived from a borrow that has since ended.
  ///
  /// # Safety
  ///
  /// `env_ptr` must point to a live `Env` owned by this isolate's thread.
  pub unsafe fn defer_gc_finalizer(
    env_ptr: *mut Env,
    cb: napi_finalize,
    data: *mut c_void,
    hint: *mut c_void,
  ) {
    // SAFETY: the caller guarantees `env_ptr` is live. This shared borrow only
    // reads the two `Rc` fields and ends before this function returns, well
    // before the deferred finalizer can mutate the `Env`.
    let env = unsafe { &*env_ptr };
    let need_schedule =
      env.ref_tracker.borrow_mut().defer(PendingNapiFinalizer {
        // GC-ready entries were already deregistered by id, and nothing can
        // look them up again.
        id: None,
        env: env_ptr as napi_env,
        cb,
        data,
        hint,
      });
    // Only the first finalizer of a GC batch schedules a drain; the rest ride
    // along on the same task (see `RefTracker::defer`).
    if need_schedule {
      let tracker = env.ref_tracker.clone();
      env.gc_finalizer_spawner.spawn(move |scope| {
        drain_gc_finalizers(scope, &tracker);
      });
    }
  }
}

/// Runs the GC-collected finalizers queued by [`Env::defer_gc_finalizer`].
/// Invoked from the event-loop task queue, so JavaScript execution is legal
/// here and finalizers may call back into JS.
fn drain_gc_finalizers(
  scope: &mut v8::PinScope,
  tracker: &RefCell<RefTracker>,
) {
  // Take the batch before running anything: a finalizer may call
  // `napi_delete_reference` (re-borrowing the tracker) or defer more
  // finalizers (which then schedule their own task).
  let ready = tracker.borrow_mut().take_gc_ready();
  for f in ready {
    // Backstop `TryCatch`: each napi entry point already traps its callback's
    // exceptions into `env.last_exception`, but keep one here so a finalizer
    // that still lets a throw escape cannot corrupt the shared event-loop
    // `HandleScope` (`V8TaskSpawner::spawn`'s contract). Recreating it per
    // finalizer clears any caught exception before the next one runs.
    v8::tc_scope!(tc, scope);
    // SAFETY: env/data/hint were captured when the addon created the reference;
    // V8 is alive and we hold a scope.
    unsafe {
      (f.cb)(f.env, f.data, f.hint);
    }
    // A throw from a finalizer usually never reaches the `TryCatch` above: the
    // napi entry point the finalizer called (e.g. `napi_call_function`)
    // records it in `env.last_exception` and returns `napi_pending_exception`
    // (see the `napi_wrap!` macro). Nobody is going to consume that here, and
    // leaving it set would make *every* later napi call on this env fail with
    // `napi_pending_exception`. Node instead reports a finalizer's uncaught
    // exception through the uncaught-exception policy and carries on, so clear
    // it — and, since a spawner task has no event-loop exception state to hand
    // it to, report it rather than dropping it silently.
    if !f.env.is_null() {
      // SAFETY: `f.env` is the `*mut Env` recorded by `defer_gc_finalizer`,
      // still live for the same reasons the callback above could use it.
      let env = unsafe { &mut *(f.env as *mut Env) };
      if let Some(exception) = env.last_exception.take() {
        let exception = v8::Local::new(tc, &exception);
        report_finalizer_exception(tc, exception);
      }
    }
    // Backstop for a throw that escaped without being recorded.
    if let Some(exception) = tc.exception() {
      report_finalizer_exception(tc, exception);
    }
  }
}

fn report_finalizer_exception(
  scope: &v8::PinScope,
  exception: v8::Local<v8::Value>,
) {
  log::error!(
    "Uncaught exception in Node-API finalizer: {}",
    exception.to_rust_string_lossy(scope)
  );
}

deno_core::extension!(deno_napi,
  ops = [
    op_napi_open
  ],
  options = {
    deno_rt_native_addon_loader: Option<DenoRtNativeAddonLoaderRc>,
  },
  state = |state, options| {
    state.put(NapiState {
      env_cleanup_hooks: Rc::new(RefCell::new(vec![])),
      ref_tracker: Rc::new(RefCell::new(RefTracker::default())),
      env_shared_ptrs: vec![],
      env_ptrs: vec![],
      napi_wrap: None,
      type_tag: None,
    });
    if let Some(loader) = options.deno_rt_native_addon_loader {
      state.put(loader);
    }
  },
);

unsafe impl Sync for NapiModuleHandle {}
unsafe impl Send for NapiModuleHandle {}

#[derive(Clone, Copy)]
struct NapiModuleHandle(*const NapiModule);

static NAPI_LOADED_MODULES: std::sync::LazyLock<
  RwLock<HashMap<PathBuf, NapiModuleHandle>>,
> = std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

#[op2(reentrant, stack_trace)]
fn op_napi_open<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  isolate: &mut v8::Isolate,
  op_state: Rc<RefCell<OpState>>,
  #[string] path: &str,
  global: v8::Local<'scope, v8::Object>,
  create_buffer: v8::Local<'scope, v8::Function>,
  report_error: v8::Local<'scope, v8::Function>,
  async_hooks_init: v8::Local<'scope, v8::Function>,
  async_hooks_before: v8::Local<'scope, v8::Function>,
  async_hooks_after: v8::Local<'scope, v8::Function>,
  async_hooks_destroy: v8::Local<'scope, v8::Function>,
) -> Result<v8::Local<'scope, v8::Value>, NApiError> {
  // We must limit the OpState borrow because this function can trigger a
  // re-borrow through the NAPI module.
  let (
    async_work_sender,
    gc_finalizer_spawner,
    cleanup_hooks,
    ref_tracker,
    external_ops_tracker,
    deno_rt_native_addon_loader,
    path,
  ) = {
    let mut op_state = op_state.borrow_mut();
    let permissions =
      op_state.borrow_mut::<deno_permissions::PermissionsContainer>();
    let path = permissions.check_ffi(Cow::Borrowed(Path::new(path)))?;
    let napi_state = op_state.borrow::<NapiState>();
    (
      op_state.borrow::<V8CrossThreadTaskSpawner>().clone(),
      op_state.borrow::<V8TaskSpawner>().clone(),
      napi_state.env_cleanup_hooks.clone(),
      napi_state.ref_tracker.clone(),
      op_state.external_ops_tracker.clone(),
      op_state.try_borrow::<DenoRtNativeAddonLoaderRc>().cloned(),
      path,
    )
  };

  // Register the uv_compat loop so that our libuv-ABI `uv_timer_*`
  // polyfills bridge onto Deno's event loop instead of degrading to
  // no-ops. The loop pointer is opaque to addons — they only pass it
  // through to other uv_* polyfill functions, which re-resolve it from
  // this thread-local in any case.
  {
    let op_state = op_state.borrow();
    if let Some(uv_loop) =
      op_state.try_borrow::<Box<deno_core::uv_compat::UvLoop>>()
    {
      let loop_ptr =
        &**uv_loop as *const deno_core::uv_compat::UvLoop as *mut _;
      crate::uv::register_default_uv_loop(loop_ptr);
    }
  }

  // Unix uv_poll forwards to the backing libuv-compatible loop.
  // `poll_scope` and `uv_loop_liveness` cover either teardown order:
  // the poll scope suppresses queued callbacks during N-API teardown while
  // the loop is still live; the liveness guard serializes loop-state access
  // with loop teardown, waiting for in-flight operations and making later
  // `uv_loop_operation_guard` calls return `None` once teardown invalidates
  // liveness.
  #[cfg(unix)]
  let (uv_loop, uv_loop_liveness, poll_scope) = {
    let op_state = op_state.borrow();
    let uv_loop = &**op_state.borrow::<Box<deno_core::uv_compat::UvLoop>>()
      as *const deno_core::uv_compat::UvLoop
      as *mut deno_core::uv_compat::uv_loop_t;
    (
      uv_loop,
      unsafe { deno_core::uv_compat::uv_loop_liveness(uv_loop) },
      unsafe { deno_core::uv_compat::new_poll_scope(uv_loop) },
    )
  };

  // Use per-isolate Private keys (like Node.js) so that objects wrapped by one
  // addon can be unwrapped by another. Lazily create on first addon load.
  let (napi_wrap, type_tag) = {
    let mut op_state_mut = op_state.borrow_mut();
    let napi_state = op_state_mut.borrow_mut::<NapiState>();
    let napi_wrap = match &napi_state.napi_wrap {
      Some(existing) => existing.clone(),
      None => {
        let name = v8::String::new(scope, "napi_wrap").unwrap();
        let key = v8::Private::new(scope, Some(name));
        let global = v8::Global::new(scope, key);
        napi_state.napi_wrap = Some(global.clone());
        global
      }
    };
    let type_tag = match &napi_state.type_tag {
      Some(existing) => existing.clone(),
      None => {
        let name = v8::String::new(scope, "type_tag").unwrap();
        let key = v8::Private::new(scope, Some(name));
        let global = v8::Global::new(scope, key);
        napi_state.type_tag = Some(global.clone());
        global
      }
    };
    (napi_wrap, type_tag)
  };

  #[allow(
    clippy::disallowed_methods,
    reason = "napi requires file path URL conversion"
  )]
  let url_filename =
    Url::from_file_path(&path).map_err(|_| NApiError::InvalidPath)?;
  let env_shared =
    EnvShared::new(napi_wrap, type_tag, format!("{url_filename}\0"));

  let ctx = scope.get_current_context();
  let mut env = Env::new(
    unsafe { isolate.as_raw_isolate_ptr() },
    v8::Global::new(scope, ctx),
    v8::Global::new(scope, global),
    v8::Global::new(scope, create_buffer),
    v8::Global::new(scope, report_error),
    v8::Global::new(scope, async_hooks_init),
    v8::Global::new(scope, async_hooks_before),
    v8::Global::new(scope, async_hooks_after),
    v8::Global::new(scope, async_hooks_destroy),
    async_work_sender,
    gc_finalizer_spawner,
    cleanup_hooks,
    ref_tracker,
    external_ops_tracker,
    #[cfg(unix)]
    uv_loop,
    #[cfg(unix)]
    uv_loop_liveness,
    #[cfg(unix)]
    poll_scope,
  );
  env.shared = Box::into_raw(Box::new(env_shared));
  // Track the EnvShared pointer so we can call instance data finalize
  // callbacks when the runtime exits. Each op_napi_open call creates a
  // fresh EnvShared, so entries are always unique.
  let env_ptr = Box::into_raw(Box::new(env));
  {
    let mut state = op_state.borrow_mut();
    let napi_state = state.borrow_mut::<NapiState>();
    napi_state
      .env_shared_ptrs
      .push(unsafe { (*env_ptr).shared });
    napi_state.env_ptrs.push(env_ptr);
  }
  let env_ptr = env_ptr as _;

  #[cfg(unix)]
  let flags = RTLD_LAZY;
  #[cfg(not(unix))]
  let flags = 0x00000008;

  let real_path = match deno_rt_native_addon_loader {
    Some(loader) => loader.load_and_resolve_path(&path)?,
    None => Cow::Borrowed(path.as_ref()),
  };

  // SAFETY: opening a DLL calls dlopen
  #[cfg(unix)]
  let library = match unsafe { Library::open(Some(real_path.as_ref()), flags) }
  {
    Ok(library) => library,
    Err(err) => {
      return Err(NApiError::LibraryLoad {
        source: err,
        path: real_path.to_path_buf(),
      });
    }
  };

  // SAFETY: opening a DLL calls dlopen
  #[cfg(not(unix))]
  let library =
    match unsafe { Library::load_with_flags(real_path.as_ref(), flags) } {
      Ok(library) => library,
      Err(err) => {
        // A `.node` that links *directly* against the Node.js binary (a regular,
        // non delay-load import of `node.exe`) cannot be loaded into any host
        // that isn't literally named `node.exe`: it expects V8/Node internal
        // symbols and libuv to be provided by that executable. Detect this and
        // surface a clear, actionable error rather than the opaque
        // `LoadLibraryExW failed` from the Windows loader. See denoland/deno#25956.
        use std::io::Read;
        let mut bytes = Vec::new();
        if std::fs::File::open(real_path.as_ref())
          .and_then(|mut f| f.read_to_end(&mut bytes))
          .is_ok()
          && pe::imports_node_executable(&bytes)
        {
          return Err(NApiError::UnsupportedNodeBinaryAddon(path.into_owned()));
        }
        return Err(NApiError::LibraryLoad {
          source: err,
          path: real_path.to_path_buf(),
        });
      }
    };

  let maybe_module = MODULE_TO_REGISTER.with(|cell| {
    let mut slot = cell.borrow_mut();
    slot.take()
  });

  // The `module.exports` object.
  let exports = v8::Object::new(scope);

  let maybe_exports = if let Some(module_to_register) = maybe_module {
    // SAFETY: napi_register_module guarantees that `module_to_register` is valid.
    let nm = unsafe { &*module_to_register };
    // A version other than `NAPI_MODULE_VERSION` (1) means this is a legacy
    // V8/nan addon registered via `node_module_register`. We can't run its
    // register function (it uses the unsupported legacy ABI), so bail out with
    // a clear error instead of crashing later. See denoland/deno#26656.
    if nm.nm_version != NAPI_MODULE_VERSION {
      return Err(NApiError::UnsupportedLegacyAddon(path.into_owned()));
    }
    NAPI_LOADED_MODULES.write().insert(
      real_path.to_path_buf(),
      NapiModuleHandle(module_to_register),
    );
    // SAFETY: we are going blind, calling the register function on the other side.
    unsafe { (nm.nm_register_func)(env_ptr, exports.into()) }
  } else if let Some(module_to_register) =
    { NAPI_LOADED_MODULES.read().get(real_path.as_ref()).copied() }
  {
    // SAFETY: this originated from `napi_register_module`, so the
    // pointer should still be valid.
    let nm = unsafe { &*module_to_register.0 };
    if nm.nm_version != NAPI_MODULE_VERSION {
      return Err(NApiError::UnsupportedLegacyAddon(path.into_owned()));
    }
    // SAFETY: we are going blind, calling the register function on the other side.
    unsafe { (nm.nm_register_func)(env_ptr, exports.into()) }
  } else {
    match unsafe {
      library.get::<napi_register_module_v1>(b"napi_register_module_v1")
    } {
      Ok(init) => {
        // Initializer callback.
        // SAFETY: we are going blind, calling the register function on the other side.
        unsafe { init(env_ptr, exports.into()) }
      }
      _ => {
        return Err(NApiError::ModuleNotFound(path.into_owned()));
      }
    }
  };

  let exports = maybe_exports.unwrap_or(exports.into());

  // NAPI addons can't be unloaded, so we're going to "forget" the library
  // object so it lives till the program exit.
  std::mem::forget(library);

  Ok(exports)
}

#[allow(clippy::print_stdout, reason = "cargo build script output")]
pub fn print_linker_flags(name: &str) {
  let symbols_path =
    include_str!(concat!(env!("OUT_DIR"), "/napi_symbol_path.txt"));

  #[cfg(target_os = "windows")]
  println!("cargo:rustc-link-arg-bin={name}=/DEF:{}", symbols_path);

  #[cfg(target_os = "macos")]
  println!(
    "cargo:rustc-link-arg-bin={name}=-Wl,-exported_symbols_list,{}",
    symbols_path,
  );

  #[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd"
  ))]
  println!(
    "cargo:rustc-link-arg-bin={name}=-Wl,--export-dynamic-symbol-list={}",
    symbols_path,
  );

  #[cfg(target_os = "android")]
  println!(
    "cargo:rustc-link-arg-bin={name}=-Wl,--export-dynamic-symbol-list={}",
    symbols_path,
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  unsafe extern "C" fn noop_finalize(
    _env: napi_env,
    _data: *mut c_void,
    _hint: *mut c_void,
  ) {
  }

  fn add_entry(tracker: &mut RefTracker) -> NapiFinalizerId {
    // The tracker only stores these pointers; it never dereferences them, so
    // nulls are fine for exercising the add/remove/drain bookkeeping.
    tracker.add(
      std::ptr::null_mut(),
      noop_finalize,
      std::ptr::null_mut(),
      std::ptr::null_mut(),
    )
  }

  // Deterministic guards for the run-once contract behind
  // https://github.com/denoland/deno/issues/36499.
  //
  // A napi_wrap finalizer can be reached by two independent paths: env teardown
  // (`run_napi_ref_finalizers` -> `take_pending`) and the GC weak callback
  // (`Reference::reset` -> `remove_ref_finalizer` -> `remove`). The tracker is
  // the single arbiter — whichever path removes the entry first runs the
  // finalizer, and the other must observe that the entry is gone and skip it.
  // These tests pin the "remove reports whether it was still pending" contract
  // that `weak_callback`'s `if was_pending` guard relies on. Unlike the
  // threadsafe-function repro, they fail 100% of the time on a regression.

  #[test]
  fn teardown_first_leaves_nothing_for_the_gc_path() {
    let mut tracker = RefTracker::default();
    let id = add_entry(&mut tracker);

    // Teardown wins the race and drains every pending finalizer.
    assert_eq!(tracker.take_pending().len(), 1);

    // A later GC weak callback tries to remove the same entry. It must report
    // `false` so `weak_callback` does not invoke the finalizer a second time.
    assert!(
      !tracker.remove(id),
      "entry drained by teardown must no longer be pending"
    );
  }

  #[test]
  fn gc_first_leaves_nothing_for_teardown() {
    let mut tracker = RefTracker::default();
    let id = add_entry(&mut tracker);

    // GC weak callback wins the race and claims the entry, running it once.
    assert!(tracker.remove(id), "first removal owns the finalizer");

    // Teardown drains afterwards and must find nothing left to run.
    assert!(
      tracker.take_pending().is_empty(),
      "entry claimed by the GC path must not be drained again at teardown"
    );

    // A redundant second removal (e.g. `Drop` after `reset`) is a no-op.
    assert!(
      !tracker.remove(id),
      "double removal must report not-pending"
    );
  }
}
