#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

pub use std::ffi::CStr;
pub use std::mem::transmute;
pub use std::os::raw::c_char;
pub use std::os::raw::c_void;
pub use std::ptr;
pub use v8;

use crate::futures::channel::mpsc;

pub use crate::runtime::PendingNapiAsyncWork;
use crate::JsRuntime;
use std::cell::RefCell;
use std::ffi::CString;

use std::thread_local;

#[cfg(unix)]
use libloading::os::unix::*;

#[cfg(windows)]
use libloading::os::windows::*;

pub type napi_status = i32;
pub type napi_env = *mut c_void;
pub type napi_value = *mut c_void;
pub type napi_callback_info = *mut c_void;
pub type napi_deferred = *mut c_void;
pub type napi_ref = *mut c_void;
pub type napi_threadsafe_function = *mut c_void;
pub type napi_handle_scope = *mut c_void;
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

thread_local! {
  pub static MODULE: RefCell<Option<*const NapiModule>> = RefCell::new(None);
  pub static ASYNC_WORK_SENDER: RefCell<Option<mpsc::UnboundedSender<PendingNapiAsyncWork>>> = RefCell::new(None);
}

type napi_addon_register_func =
  extern "C" fn(env: napi_env, exports: napi_value) -> napi_value;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct NapiModule {
  pub nm_version: i32,
  pub nm_flags: u32,
  nm_filename: *const c_char,
  pub nm_register_func: napi_addon_register_func,
  nm_modname: *const c_char,
  nm_priv: *mut c_void,
  reserved: [*mut c_void; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
  InvalidArg,
  ObjectExpected,
  StringExpected,
  NameExpected,
  FunctionExpected,
  NumberExpected,
  BooleanExpected,
  ArrayExpected,
  GenericFailure,
  PendingException,
  Cancelled,
  EscapeCalledTwice,
  HandleScopeMismatch,
  CallbackScopeMismatch,
  QueueFull,
  Closing,
  BigIntExpected,
  DateExpected,
  ArrayBufferExpected,
  DetachableArraybufferExpected,
  WouldDeadlock,
}

pub type Result = std::result::Result<(), Error>;

impl From<Error> for napi_status {
  fn from(error: Error) -> Self {
    match error {
      Error::InvalidArg => napi_invalid_arg,
      Error::ObjectExpected => napi_object_expected,
      Error::StringExpected => napi_string_expected,
      Error::NameExpected => napi_name_expected,
      Error::FunctionExpected => napi_function_expected,
      Error::NumberExpected => napi_number_expected,
      Error::BooleanExpected => napi_boolean_expected,
      Error::ArrayExpected => napi_array_expected,
      Error::GenericFailure => napi_generic_failure,
      Error::PendingException => napi_pending_exception,
      Error::Cancelled => napi_cancelled,
      Error::EscapeCalledTwice => napi_escape_called_twice,
      Error::HandleScopeMismatch => napi_handle_scope_mismatch,
      Error::CallbackScopeMismatch => napi_callback_scope_mismatch,
      Error::QueueFull => napi_queue_full,
      Error::Closing => napi_closing,
      Error::BigIntExpected => napi_bigint_expected,
      Error::DateExpected => napi_date_expected,
      Error::ArrayBufferExpected => napi_arraybuffer_expected,
      Error::DetachableArraybufferExpected => {
        napi_detachable_arraybuffer_expected
      }
      Error::WouldDeadlock => napi_would_deadlock,
    }
  }
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

pub type napi_callback =
  unsafe extern "C" fn(env: napi_env, info: napi_callback_info) -> napi_value;

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
pub struct napi_property_descriptor {
  pub utf8name: *const c_char,
  pub name: napi_value,
  pub method: napi_callback,
  pub getter: napi_callback,
  pub setter: napi_callback,
  pub value: napi_value,
  pub attributes: napi_property_attributes,
  pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug)]
pub struct napi_extended_error_info {
  pub error_message: *const c_char,
  pub engine_reserved: *mut c_void,
  pub engine_error_code: i32,
  pub status_code: napi_status,
}

#[repr(C)]
#[derive(Debug)]
pub struct napi_node_version {
  pub major: u32,
  pub minor: u32,
  pub patch: u32,
  pub release: *const c_char,
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
  // pub struct Env<'a, 'b> {
  // pub scope: &'a mut v8::HandleScope<'b>,
  pub context: v8::Global<v8::Context>,
  pub isolate_ptr: *mut v8::OwnedIsolate,
  pub open_handle_scopes: usize,
  pub shared: *mut EnvShared,
  pub async_work_sender: mpsc::UnboundedSender<PendingNapiAsyncWork>,
  pub threadsafe_function_sender:
    mpsc::UnboundedSender<ThreadSafeFunctionStatus>,
}

// unsafe impl Send for Env<'_, '_> {}
// unsafe impl Sync for Env<'_, '_> {}
unsafe impl Send for Env {}
unsafe impl Sync for Env {}

// impl<'a, 'b> Env<'a, 'b> {
impl Env {
  pub fn new(
    isolate_ptr: *mut v8::OwnedIsolate,
    // scope: &'a mut v8::HandleScope<'b>,
    context: v8::Global<v8::Context>,
    sender: mpsc::UnboundedSender<PendingNapiAsyncWork>,
    threadsafe_function_sender: mpsc::UnboundedSender<ThreadSafeFunctionStatus>,
  ) -> Self {
    let sc = sender.clone();
    ASYNC_WORK_SENDER.with(|s| {
      s.replace(Some(sc));
    });
    Self {
      // scope,
      isolate_ptr,
      context,
      shared: std::ptr::null_mut(),
      open_handle_scopes: 0,
      async_work_sender: sender,
      threadsafe_function_sender,
    }
  }

  // TODO(@littledivy): Do we even need this?
  pub fn with_new_scope(
    &self,
    // scope: &'a mut v8::HandleScope<'b>,
    sender: mpsc::UnboundedSender<PendingNapiAsyncWork>,
  ) -> Self {
    let sc = sender.clone();
    ASYNC_WORK_SENDER.with(|s| {
      s.replace(Some(sc));
    });
    Self {
      // scope,
      isolate_ptr: self.isolate_ptr,
      shared: self.shared,
      open_handle_scopes: self.open_handle_scopes,
      async_work_sender: sender,
      threadsafe_function_sender: self.threadsafe_function_sender.clone(),
      context: self.context.clone(),
    }
  }

  pub fn shared(&self) -> &EnvShared {
    unsafe { &*self.shared }
  }

  pub fn shared_mut(&mut self) -> &mut EnvShared {
    unsafe { &mut *self.shared }
  }

  pub fn add_async_work(&mut self, async_work: PendingNapiAsyncWork) {
    self.async_work_sender.unbounded_send(async_work).unwrap();
  }

  // TODO(@littledivy): Painful hack. ouch.
  pub fn scope(&self) -> v8::CallbackScope {
    let persistent_context = self.context.clone();
    let data = persistent_context.into_raw();
    let context = unsafe {
      transmute::<NonNull<v8::Context>, v8::Local<v8::Context>>(data)
    };
    use core::ptr::NonNull;
    unsafe { v8::Global::from_raw(&mut **self.isolate_ptr, data) };
    unsafe { v8::CallbackScope::new(context) }
  }
}

pub fn napi_open_func<'s>(
  scope: &mut v8::HandleScope<'s>,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let external =
    v8::Local::<v8::External>::try_from(args.data().unwrap()).unwrap();
  let value = external.value() as *mut v8::OwnedIsolate;

  let napi_wrap_name = v8::String::new(scope, "napi_wrap").unwrap();
  let napi_wrap = v8::Private::new(scope, Some(napi_wrap_name));
  let napi_wrap = v8::Local::new(scope, napi_wrap);
  let napi_wrap = v8::Global::new(scope, napi_wrap);

  let path = args.get(0).to_string(scope).unwrap();
  let path = path.to_rust_string_lossy(scope);

  let exports = v8::Object::new(scope);

  // We need complete control over the env object's lifetime
  // so we'll use explicit allocation for it, so that it doesn't
  // die before the module itself. Using struct & their pointers
  // resulted in a use-after-free situation which turned out to be
  // unfixable, so here we are.
  let env_shared_ptr = unsafe {
    std::alloc::alloc(std::alloc::Layout::new::<EnvShared>()) as *mut EnvShared
  };
  let mut env_shared = EnvShared::new(napi_wrap);
  let cstr = CString::new(path.clone()).unwrap();
  env_shared.filename = cstr.as_ptr();
  std::mem::forget(cstr);
  unsafe {
    env_shared_ptr.write(env_shared);
  }

  let state_rc = JsRuntime::state(scope);
  let async_work_sender = state_rc.borrow().napi_async_work_sender.clone();
  let tsfn_sender = state_rc.borrow().napi_threadsafe_function_sender.clone();
  let env_ptr =
    unsafe { std::alloc::alloc(std::alloc::Layout::new::<Env>()) as napi_env };
  let ctx = scope.get_current_context();
  let mut env = Env::new(
    value,
    v8::Global::new(scope, ctx),
    async_work_sender,
    tsfn_sender,
  );
  env.shared = env_shared_ptr;
  unsafe {
    (env_ptr as *mut Env).write(env);
  }

  #[cfg(unix)]
  let flags = RTLD_LAZY;
  #[cfg(not(unix))]
  let flags = 0x00000008;

  #[cfg(unix)]
  let library = match unsafe { Library::open(Some(&path), flags) } {
    Ok(lib) => lib,
    Err(e) => {
      let message = v8::String::new(scope, &e.to_string()).unwrap();
      let error = v8::Exception::type_error(scope, message);
      scope.throw_exception(error);
      return;
    }
  };

  #[cfg(not(unix))]
  let library = match unsafe { Library::load_with_flags(&path, flags) } {
    Ok(lib) => lib,
    Err(e) => {
      let message = v8::String::new(scope, &e.to_string()).unwrap();
      let error = v8::Exception::type_error(scope, message);
      scope.throw_exception(error);
      return;
    }
  };

  MODULE.with(|cell| {
    let slot = *cell.borrow();
    match slot {
      Some(nm) => {
        let nm = unsafe { &*nm };
        assert_eq!(nm.nm_version, 1);
        let exports = unsafe {
          (nm.nm_register_func)(env_ptr, std::mem::transmute(exports))
        };

        let exports: v8::Local<v8::Value> =
          unsafe { std::mem::transmute(exports) };
        rv.set(exports);
      }
      None => {
        // Initializer callback.
        let init = unsafe {
          library
            .get::<unsafe extern "C" fn(
              env: napi_env,
              exports: napi_value,
            ) -> napi_value>(b"napi_register_module_v1")
            .expect("napi_register_module_v1 not found")
        };

        unsafe { init(env_ptr, std::mem::transmute(exports)) };
        rv.set(exports.into());
      }
    }
  });

  std::mem::forget(library);
}
