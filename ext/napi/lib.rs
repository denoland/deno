// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(clippy::undocumented_unsafe_blocks)]
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
use std::cell::RefCell;
use std::collections::HashMap;
pub use std::ffi::CStr;
pub use std::os::raw::c_char;
pub use std::os::raw::c_void;
use std::path::PathBuf;
pub use std::ptr;
use std::rc::Rc;
use std::thread_local;

use deno_core::op2;
use deno_core::parking_lot::RwLock;
use deno_core::url::Url;
// Expose common stuff for ease of use.
// `use deno_napi::*`
pub use deno_core::v8;
use deno_core::ExternalOpsTracker;
use deno_core::OpState;
use deno_core::V8CrossThreadTaskSpawner;
use deno_permissions::PermissionCheckError;
pub use denort_helper::DenoRtNativeAddonLoader;
pub use denort_helper::DenoRtNativeAddonLoaderRc;
#[cfg(unix)]
use libloading::os::unix::*;
#[cfg(windows)]
use libloading::os::windows::*;
pub use value::napi_value;

pub mod function;
mod value;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum NApiError {
  #[class(type)]
  #[error("Invalid path")]
  InvalidPath,
  #[class(type)]
  #[error(transparent)]
  DenoRtLoad(#[from] denort_helper::LoadError),
  #[class(type)]
  #[error(transparent)]
  LibLoading(#[from] libloading::Error),
  #[class(type)]
  #[error("Unable to find register Node-API module at {}", .0.display())]
  ModuleNotFound(PathBuf),
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
  pub error_code: napi_status,
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

pub struct NapiState {
  // Thread safe functions.
  pub env_cleanup_hooks: Rc<RefCell<Vec<(napi_cleanup_hook, *mut c_void)>>>,
}

impl Drop for NapiState {
  fn drop(&mut self) {
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
  pub isolate_ptr: *mut v8::Isolate,
  pub open_handle_scopes: usize,
  pub shared: *mut EnvShared,
  pub async_work_sender: V8CrossThreadTaskSpawner,
  cleanup_hooks: Rc<RefCell<Vec<(napi_cleanup_hook, *mut c_void)>>>,
  external_ops_tracker: ExternalOpsTracker,
  pub last_error: napi_extended_error_info,
  pub last_exception: Option<v8::Global<v8::Value>>,
  pub global: v8::Global<v8::Object>,
  pub buffer_constructor: v8::Global<v8::Function>,
  pub report_error: v8::Global<v8::Function>,
}

unsafe impl Send for Env {}
unsafe impl Sync for Env {}

impl Env {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    isolate_ptr: *mut v8::Isolate,
    context: v8::Global<v8::Context>,
    global: v8::Global<v8::Object>,
    buffer_constructor: v8::Global<v8::Function>,
    report_error: v8::Global<v8::Function>,
    sender: V8CrossThreadTaskSpawner,
    cleanup_hooks: Rc<RefCell<Vec<(napi_cleanup_hook, *mut c_void)>>>,
    external_ops_tracker: ExternalOpsTracker,
  ) -> Self {
    Self {
      isolate_ptr,
      context: context.into_raw(),
      global,
      buffer_constructor,
      report_error,
      shared: std::ptr::null_mut(),
      open_handle_scopes: 0,
      async_work_sender: sender,
      cleanup_hooks,
      external_ops_tracker,
      last_error: napi_extended_error_info {
        error_message: std::ptr::null(),
        engine_reserved: std::ptr::null_mut(),
        engine_error_code: 0,
        error_code: napi_ok,
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
    unsafe { &mut *self.isolate_ptr }
  }

  #[inline]
  pub fn scope(&self) -> v8::CallbackScope {
    // SAFETY: `v8::Local` is always non-null pointer; the `HandleScope` is
    // already on the stack, but we don't have access to it.
    let context = unsafe {
      std::mem::transmute::<NonNull<v8::Context>, v8::Local<v8::Context>>(
        self.context,
      )
    };
    // SAFETY: there must be a `HandleScope` on the stack, this is ensured because
    // we are in a V8 callback or the module has already opened a `HandleScope`
    // using `napi_open_handle_scope`.
    unsafe { v8::CallbackScope::new(context) }
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
}

deno_core::extension!(deno_napi,
  parameters = [P: NapiPermissions],
  ops = [
    op_napi_open<P>
  ],
  options = {
    deno_rt_native_addon_loader: Option<DenoRtNativeAddonLoaderRc>,
  },
  state = |state, options| {
    state.put(NapiState {
      env_cleanup_hooks: Rc::new(RefCell::new(vec![])),
    });
    if let Some(loader) = options.deno_rt_native_addon_loader {
      state.put(loader);
    }
  },
);

pub trait NapiPermissions {
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check(&mut self, path: &str) -> Result<PathBuf, PermissionCheckError>;
}

// NOTE(bartlomieju): for now, NAPI uses `--allow-ffi` flag, but that might
// change in the future.
impl NapiPermissions for deno_permissions::PermissionsContainer {
  #[inline(always)]
  fn check(&mut self, path: &str) -> Result<PathBuf, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_ffi(self, path)
  }
}

unsafe impl Sync for NapiModuleHandle {}
unsafe impl Send for NapiModuleHandle {}

#[derive(Clone, Copy)]
struct NapiModuleHandle(*const NapiModule);

static NAPI_LOADED_MODULES: std::sync::LazyLock<
  RwLock<HashMap<PathBuf, NapiModuleHandle>>,
> = std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

#[op2(reentrant, stack_trace)]
fn op_napi_open<NP, 'scope>(
  scope: &mut v8::HandleScope<'scope>,
  isolate: *mut v8::Isolate,
  op_state: Rc<RefCell<OpState>>,
  #[string] path: String,
  global: v8::Local<'scope, v8::Object>,
  buffer_constructor: v8::Local<'scope, v8::Function>,
  report_error: v8::Local<'scope, v8::Function>,
) -> Result<v8::Local<'scope, v8::Value>, NApiError>
where
  NP: NapiPermissions + 'static,
{
  // We must limit the OpState borrow because this function can trigger a
  // re-borrow through the NAPI module.
  let (
    async_work_sender,
    cleanup_hooks,
    external_ops_tracker,
    deno_rt_native_addon_loader,
    path,
  ) = {
    let mut op_state = op_state.borrow_mut();
    let permissions = op_state.borrow_mut::<NP>();
    let path = permissions.check(&path)?;
    let napi_state = op_state.borrow::<NapiState>();
    (
      op_state.borrow::<V8CrossThreadTaskSpawner>().clone(),
      napi_state.env_cleanup_hooks.clone(),
      op_state.external_ops_tracker.clone(),
      op_state.try_borrow::<DenoRtNativeAddonLoaderRc>().cloned(),
      path,
    )
  };

  let napi_wrap_name = v8::String::new(scope, "napi_wrap").unwrap();
  let napi_wrap = v8::Private::new(scope, Some(napi_wrap_name));
  let napi_wrap = v8::Global::new(scope, napi_wrap);

  let type_tag_name = v8::String::new(scope, "type_tag").unwrap();
  let type_tag = v8::Private::new(scope, Some(type_tag_name));
  let type_tag = v8::Global::new(scope, type_tag);

  let url_filename =
    Url::from_file_path(&path).map_err(|_| NApiError::InvalidPath)?;
  let env_shared =
    EnvShared::new(napi_wrap, type_tag, format!("{url_filename}\0"));

  let ctx = scope.get_current_context();
  let mut env = Env::new(
    isolate,
    v8::Global::new(scope, ctx),
    v8::Global::new(scope, global),
    v8::Global::new(scope, buffer_constructor),
    v8::Global::new(scope, report_error),
    async_work_sender,
    cleanup_hooks,
    external_ops_tracker,
  );
  env.shared = Box::into_raw(Box::new(env_shared));
  let env_ptr = Box::into_raw(Box::new(env)) as _;

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
  let library = unsafe { Library::open(Some(real_path.as_ref()), flags) }?;

  // SAFETY: opening a DLL calls dlopen
  #[cfg(not(unix))]
  let library = unsafe { Library::load_with_flags(real_path.as_ref(), flags) }?;

  let maybe_module = MODULE_TO_REGISTER.with(|cell| {
    let mut slot = cell.borrow_mut();
    slot.take()
  });

  // The `module.exports` object.
  let exports = v8::Object::new(scope);

  let maybe_exports = if let Some(module_to_register) = maybe_module {
    NAPI_LOADED_MODULES
      .write()
      .insert(path, NapiModuleHandle(module_to_register));
    // SAFETY: napi_register_module guarantees that `module_to_register` is valid.
    let nm = unsafe { &*module_to_register };
    assert_eq!(nm.nm_version, 1);
    // SAFETY: we are going blind, calling the register function on the other side.
    unsafe { (nm.nm_register_func)(env_ptr, exports.into()) }
  } else if let Some(module_to_register) =
    { NAPI_LOADED_MODULES.read().get(&path).copied() }
  {
    // SAFETY: this originated from `napi_register_module`, so the
    // pointer should still be valid.
    let nm = unsafe { &*module_to_register.0 };
    assert_eq!(nm.nm_version, 1);
    // SAFETY: we are going blind, calling the register function on the other side.
    unsafe { (nm.nm_register_func)(env_ptr, exports.into()) }
  } else if let Ok(init) = unsafe {
    library.get::<napi_register_module_v1>(b"napi_register_module_v1")
  } {
    // Initializer callback.
    // SAFETY: we are going blind, calling the register function on the other side.
    unsafe { init(env_ptr, exports.into()) }
  } else {
    return Err(NApiError::ModuleNotFound(path));
  };

  let exports = maybe_exports.unwrap_or(exports.into());

  // NAPI addons can't be unloaded, so we're going to "forget" the library
  // object so it lives till the program exit.
  std::mem::forget(library);

  Ok(exports)
}

#[allow(clippy::print_stdout)]
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
