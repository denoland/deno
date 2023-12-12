// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::missing_safety_doc)]

use core::ptr::NonNull;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::op2;
use deno_core::parking_lot::Mutex;
use deno_core::OpState;
use deno_core::V8CrossThreadTaskSpawner;
use std::cell::RefCell;
use std::ffi::CString;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::thread_local;

#[cfg(unix)]
use libloading::os::unix::*;

#[cfg(windows)]
use libloading::os::windows::*;

// Expose common stuff for ease of use.
// `use deno_napi::*`
pub use deno_core::v8;
pub use std::ffi::CStr;
pub use std::mem::transmute;
pub use std::os::raw::c_char;
pub use std::os::raw::c_void;
pub use std::ptr;
pub use value::napi_value;

pub mod function;
mod value;

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

pub const NAPI_AUTO_LENGTH: usize = usize::MAX;

thread_local! {
  pub static MODULE_TO_REGISTER: RefCell<Option<*const NapiModule>> = RefCell::new(None);
}

type napi_addon_register_func =
  extern "C" fn(env: napi_env, exports: napi_value) -> napi_value;

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
pub const napi_tsfn_abortext: napi_threadsafe_function_release_mode = 1;

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

pub struct napi_type_tag {
  pub lower: u64,
  pub upper: u64,
}

pub type napi_callback = Option<
  unsafe extern "C" fn(
    env: napi_env,
    info: napi_callback_info,
  ) -> napi_value<'static>,
>;

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

pub type napi_async_cleanup_hook =
  unsafe extern "C" fn(env: napi_env, data: *mut c_void);

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
  pub method: napi_callback,
  pub getter: napi_callback,
  pub setter: napi_callback,
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

pub type ThreadsafeFunctionRefCounters = Vec<(usize, Arc<AtomicUsize>)>;
pub struct NapiState {
  // Thread safe functions.
  pub active_threadsafe_functions: usize,
  pub threadsafe_function_receiver:
    mpsc::UnboundedReceiver<ThreadSafeFunctionStatus>,
  pub threadsafe_function_sender:
    mpsc::UnboundedSender<ThreadSafeFunctionStatus>,
  pub env_cleanup_hooks:
    Rc<RefCell<Vec<(extern "C" fn(*const c_void), *const c_void)>>>,
  pub tsfn_ref_counters: Arc<Mutex<ThreadsafeFunctionRefCounters>>,
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
        .any(|pair| pair.0 == hook.0 && pair.1 == hook.1)
      {
        continue;
      }

      (hook.0)(hook.1);
      {
        self
          .env_cleanup_hooks
          .borrow_mut()
          .retain(|pair| !(pair.0 == hook.0 && pair.1 == hook.1));
      }
    }
  }
}
#[repr(C)]
#[derive(Debug)]
/// Env that is shared between all contexts in same native module.
pub struct EnvShared {
  pub instance_data: *mut c_void,
  pub data_finalize: Option<napi_finalize>,
  pub data_finalize_hint: *mut c_void,
  pub napi_wrap: v8::Global<v8::Private>,
  pub finalize: Option<napi_finalize>,
  pub finalize_hint: *mut c_void,
  pub filename: *const c_char,
}

impl EnvShared {
  pub fn new(napi_wrap: v8::Global<v8::Private>) -> Self {
    Self {
      instance_data: std::ptr::null_mut(),
      data_finalize: None,
      data_finalize_hint: std::ptr::null_mut(),
      napi_wrap,
      finalize: None,
      finalize_hint: std::ptr::null_mut(),
      filename: std::ptr::null(),
    }
  }
}

pub enum ThreadSafeFunctionStatus {
  Alive,
  Dead,
}

#[repr(C)]
pub struct Env {
  context: NonNull<v8::Context>,
  pub isolate_ptr: *mut v8::OwnedIsolate,
  pub open_handle_scopes: usize,
  pub shared: *mut EnvShared,
  pub async_work_sender: V8CrossThreadTaskSpawner,
  pub threadsafe_function_sender:
    mpsc::UnboundedSender<ThreadSafeFunctionStatus>,
  pub cleanup_hooks:
    Rc<RefCell<Vec<(extern "C" fn(*const c_void), *const c_void)>>>,
  pub tsfn_ref_counters: Arc<Mutex<ThreadsafeFunctionRefCounters>>,
  pub last_error: napi_extended_error_info,
  pub global: NonNull<v8::Value>,
}

unsafe impl Send for Env {}
unsafe impl Sync for Env {}

impl Env {
  pub fn new(
    isolate_ptr: *mut v8::OwnedIsolate,
    context: v8::Global<v8::Context>,
    global: v8::Global<v8::Value>,
    sender: V8CrossThreadTaskSpawner,
    threadsafe_function_sender: mpsc::UnboundedSender<ThreadSafeFunctionStatus>,
    cleanup_hooks: Rc<
      RefCell<Vec<(extern "C" fn(*const c_void), *const c_void)>>,
    >,
    tsfn_ref_counters: Arc<Mutex<ThreadsafeFunctionRefCounters>>,
  ) -> Self {
    Self {
      isolate_ptr,
      context: context.into_raw(),
      global: global.into_raw(),
      shared: std::ptr::null_mut(),
      open_handle_scopes: 0,
      async_work_sender: sender,
      threadsafe_function_sender,
      cleanup_hooks,
      tsfn_ref_counters,
      last_error: napi_extended_error_info {
        error_message: std::ptr::null(),
        engine_reserved: std::ptr::null_mut(),
        engine_error_code: 0,
        error_code: napi_ok,
      },
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
  pub fn isolate(&mut self) -> &mut v8::OwnedIsolate {
    // SAFETY: Lifetime of `OwnedIsolate` is longer than `Env`.
    unsafe { &mut *self.isolate_ptr }
  }

  #[inline]
  pub fn scope(&self) -> v8::CallbackScope {
    // SAFETY: `v8::Local` is always non-null pointer; the `HandleScope` is
    // already on the stack, but we don't have access to it.
    let context = unsafe {
      transmute::<NonNull<v8::Context>, v8::Local<v8::Context>>(self.context)
    };
    // SAFETY: there must be a `HandleScope` on the stack, this is ensured because
    // we are in a V8 callback or the module has already opened a `HandleScope`
    // using `napi_open_handle_scope`.
    unsafe { v8::CallbackScope::new(context) }
  }

  pub fn add_threadsafe_function_ref_counter(
    &mut self,
    id: usize,
    counter: Arc<AtomicUsize>,
  ) {
    let mut counters = self.tsfn_ref_counters.lock();
    assert!(!counters.iter().any(|(i, _)| *i == id));
    counters.push((id, counter));
  }

  pub fn remove_threadsafe_function_ref_counter(&mut self, id: usize) {
    let mut counters = self.tsfn_ref_counters.lock();
    let index = counters.iter().position(|(i, _)| *i == id).unwrap();
    counters.remove(index);
  }
}

deno_core::extension!(deno_napi,
  parameters = [P: NapiPermissions],
  ops = [
    op_napi_open<P>
  ],
  state = |state| {
    let (threadsafe_function_sender, threadsafe_function_receiver) =
      mpsc::unbounded::<ThreadSafeFunctionStatus>();
    state.put(NapiState {
      threadsafe_function_sender,
      threadsafe_function_receiver,
      active_threadsafe_functions: 0,
      env_cleanup_hooks: Rc::new(RefCell::new(vec![])),
      tsfn_ref_counters: Arc::new(Mutex::new(vec![])),
    });
  },
);

pub trait NapiPermissions {
  fn check(&mut self, path: Option<&Path>)
    -> std::result::Result<(), AnyError>;
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointer Env.
/// - The caller must ensure that the pointer is valid.
/// - The caller must ensure that the pointer is not freed.
pub unsafe fn weak_local(
  env_ptr: *mut Env,
  value: v8::Local<v8::Value>,
  data: *mut c_void,
  finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
) -> Option<v8::Local<v8::Value>> {
  use std::cell::Cell;

  let env = &mut *env_ptr;

  let weak_ptr = Rc::new(Cell::new(None));
  let scope = &mut env.scope();

  let weak = v8::Weak::with_finalizer(
    scope,
    value,
    Box::new({
      let weak_ptr = weak_ptr.clone();
      move |isolate| {
        finalize_cb(env_ptr as _, data as _, finalize_hint as _);

        // Self-deleting weak.
        if let Some(weak_ptr) = weak_ptr.get() {
          let weak: v8::Weak<v8::Value> =
            unsafe { v8::Weak::from_raw(isolate, Some(weak_ptr)) };
          drop(weak);
        }
      }
    }),
  );

  let value = weak.to_local(scope);
  let raw = weak.into_raw();
  weak_ptr.set(raw);

  value
}

#[op2]
fn op_napi_open<NP, 'scope>(
  scope: &mut v8::HandleScope<'scope>,
  op_state: &mut OpState,
  #[string] path: String,
  global: v8::Local<'scope, v8::Value>,
) -> std::result::Result<v8::Local<'scope, v8::Value>, AnyError>
where
  NP: NapiPermissions + 'static,
{
  let permissions = op_state.borrow_mut::<NP>();
  permissions.check(Some(&PathBuf::from(&path)))?;
  let (
    async_work_sender,
    tsfn_sender,
    isolate_ptr,
    cleanup_hooks,
    tsfn_ref_counters,
  ) = {
    let napi_state = op_state.borrow::<NapiState>();
    let isolate_ptr = op_state.borrow::<*mut v8::OwnedIsolate>();
    (
      op_state.borrow::<V8CrossThreadTaskSpawner>().clone(),
      napi_state.threadsafe_function_sender.clone(),
      *isolate_ptr,
      napi_state.env_cleanup_hooks.clone(),
      napi_state.tsfn_ref_counters.clone(),
    )
  };

  let napi_wrap_name = v8::String::new(scope, "napi_wrap").unwrap();
  let napi_wrap = v8::Private::new(scope, Some(napi_wrap_name));
  let napi_wrap = v8::Global::new(scope, napi_wrap);

  // The `module.exports` object.
  let exports = v8::Object::new(scope);

  let mut env_shared = EnvShared::new(napi_wrap);
  let cstr = CString::new(&*path).unwrap();
  env_shared.filename = cstr.as_ptr();
  std::mem::forget(cstr);

  let ctx = scope.get_current_context();
  let mut env = Env::new(
    isolate_ptr,
    v8::Global::new(scope, ctx),
    v8::Global::new(scope, global),
    async_work_sender,
    tsfn_sender,
    cleanup_hooks,
    tsfn_ref_counters,
  );
  env.shared = Box::into_raw(Box::new(env_shared));
  let env_ptr = Box::into_raw(Box::new(env)) as _;

  #[cfg(unix)]
  let flags = RTLD_LAZY;
  #[cfg(not(unix))]
  let flags = 0x00000008;

  // SAFETY: opening a DLL calls dlopen
  #[cfg(unix)]
  let library = match unsafe { Library::open(Some(&path), flags) } {
    Ok(lib) => lib,
    Err(e) => return Err(type_error(e.to_string())),
  };

  // SAFETY: opening a DLL calls dlopen
  #[cfg(not(unix))]
  let library = match unsafe { Library::load_with_flags(&path, flags) } {
    Ok(lib) => lib,
    Err(e) => return Err(type_error(e.to_string())),
  };

  let maybe_module = MODULE_TO_REGISTER.with(|cell| {
    let mut slot = cell.borrow_mut();
    slot.take()
  });

  if let Some(module_to_register) = maybe_module {
    // SAFETY: napi_register_module guarantees that `module_to_register` is valid.
    let nm = unsafe { &*module_to_register };
    assert_eq!(nm.nm_version, 1);
    // SAFETY: we are going blind, calling the register function on the other side.
    let maybe_exports = unsafe {
      (nm.nm_register_func)(
        env_ptr,
        std::mem::transmute::<v8::Local<v8::Value>, napi_value>(exports.into()),
      )
    };

    let exports = if maybe_exports.is_some() {
      // SAFETY: v8::Local is a pointer to a value and napi_value is also a pointer
      // to a value, they have the same layout
      unsafe {
        std::mem::transmute::<napi_value, v8::Local<v8::Value>>(maybe_exports)
      }
    } else {
      exports.into()
    };

    // NAPI addons can't be unloaded, so we're going to "forget" the library
    // object so it lives till the program exit.
    std::mem::forget(library);
    return Ok(exports);
  }

  // Initializer callback.
  // SAFETY: we are going blind, calling the register function on the other side.

  let maybe_exports = unsafe {
    let Ok(init) = library
      .get::<unsafe extern "C" fn(
        env: napi_env,
        exports: napi_value,
      ) -> napi_value>(b"napi_register_module_v1") else {
        return Err(type_error(format!("Unable to find napi_register_module_v1 symbol in {}", path)));
      };
    init(
      env_ptr,
      std::mem::transmute::<v8::Local<v8::Value>, napi_value>(exports.into()),
    )
  };

  let exports = if maybe_exports.is_some() {
    // SAFETY: v8::Local is a pointer to a value and napi_value is also a pointer
    // to a value, they have the same layout
    unsafe {
      std::mem::transmute::<napi_value, v8::Local<v8::Value>>(maybe_exports)
    }
  } else {
    exports.into()
  };

  // NAPI addons can't be unloaded, so we're going to "forget" the library
  // object so it lives till the program exit.
  std::mem::forget(library);
  Ok(exports)
}
