// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::missing_safety_doc)]

use core::ptr::NonNull;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::futures::StreamExt;
use deno_core::op;
use deno_core::serde_v8;
use deno_core::Extension;
use deno_core::OpState;
use std::cell::RefCell;
use std::ffi::CString;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::task::Poll;
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
  pub static MODULE: RefCell<Option<*const NapiModule>> = RefCell::new(None);
  pub static ASYNC_WORK_SENDER: RefCell<Option<mpsc::UnboundedSender<PendingNapiAsyncWork>>> = RefCell::new(None);
  pub static THREAD_SAFE_FN_SENDER: RefCell<Option<mpsc::UnboundedSender<ThreadSafeFunctionStatus>>> = RefCell::new(None);
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[allow(clippy::from_over_into)]
impl Into<Error> for napi_status {
  fn into(self) -> Error {
    match self {
      napi_invalid_arg => Error::InvalidArg,
      napi_object_expected => Error::ObjectExpected,
      napi_string_expected => Error::StringExpected,
      napi_name_expected => Error::NameExpected,
      napi_function_expected => Error::FunctionExpected,
      napi_number_expected => Error::NumberExpected,
      napi_boolean_expected => Error::BooleanExpected,
      napi_array_expected => Error::ArrayExpected,
      napi_generic_failure => Error::GenericFailure,
      napi_pending_exception => Error::PendingException,
      napi_cancelled => Error::Cancelled,
      napi_escape_called_twice => Error::EscapeCalledTwice,
      napi_handle_scope_mismatch => Error::HandleScopeMismatch,
      napi_callback_scope_mismatch => Error::CallbackScopeMismatch,
      napi_queue_full => Error::QueueFull,
      napi_closing => Error::Closing,
      napi_bigint_expected => Error::BigIntExpected,
      napi_date_expected => Error::DateExpected,
      napi_arraybuffer_expected => Error::ArrayBufferExpected,
      napi_detachable_arraybuffer_expected => {
        Error::DetachableArraybufferExpected
      }
      napi_would_deadlock => Error::WouldDeadlock,
      _ => unreachable!(),
    }
  }
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

pub type PendingNapiAsyncWork = Box<dyn FnOnce()>;
pub type ThreadsafeFunctionRefCounters = Vec<(usize, Arc<AtomicUsize>)>;
pub struct NapiState {
  // Async tasks.
  pub pending_async_work: Vec<PendingNapiAsyncWork>,
  pub async_work_sender: mpsc::UnboundedSender<PendingNapiAsyncWork>,
  pub async_work_receiver: mpsc::UnboundedReceiver<PendingNapiAsyncWork>,
  // Thread safe functions.
  pub active_threadsafe_functions: usize,
  pub threadsafe_function_receiver:
    mpsc::UnboundedReceiver<ThreadSafeFunctionStatus>,
  pub threadsafe_function_sender:
    mpsc::UnboundedSender<ThreadSafeFunctionStatus>,
  pub env_cleanup_hooks:
    Rc<RefCell<Vec<(extern "C" fn(*const c_void), *const c_void)>>>,
  pub tsfn_ref_counters: Rc<RefCell<ThreadsafeFunctionRefCounters>>,
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
  pub async_work_sender: mpsc::UnboundedSender<PendingNapiAsyncWork>,
  pub threadsafe_function_sender:
    mpsc::UnboundedSender<ThreadSafeFunctionStatus>,
  pub cleanup_hooks:
    Rc<RefCell<Vec<(extern "C" fn(*const c_void), *const c_void)>>>,
  pub tsfn_ref_counters: Rc<RefCell<ThreadsafeFunctionRefCounters>>,
  pub last_error: napi_extended_error_info,
}

unsafe impl Send for Env {}
unsafe impl Sync for Env {}

impl Env {
  pub fn new(
    isolate_ptr: *mut v8::OwnedIsolate,
    context: v8::Global<v8::Context>,
    sender: mpsc::UnboundedSender<PendingNapiAsyncWork>,
    threadsafe_function_sender: mpsc::UnboundedSender<ThreadSafeFunctionStatus>,
    cleanup_hooks: Rc<
      RefCell<Vec<(extern "C" fn(*const c_void), *const c_void)>>,
    >,
    tsfn_ref_counters: Rc<RefCell<ThreadsafeFunctionRefCounters>>,
  ) -> Self {
    let sc = sender.clone();
    ASYNC_WORK_SENDER.with(|s| {
      s.replace(Some(sc));
    });
    let ts = threadsafe_function_sender.clone();
    THREAD_SAFE_FN_SENDER.with(|s| {
      s.replace(Some(ts));
    });

    Self {
      isolate_ptr,
      context: context.into_raw(),
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

  pub fn add_async_work(&mut self, async_work: PendingNapiAsyncWork) {
    self.async_work_sender.unbounded_send(async_work).unwrap();
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
    let mut counters = self.tsfn_ref_counters.borrow_mut();
    assert!(!counters.iter().any(|(i, _)| *i == id));
    counters.push((id, counter));
  }

  pub fn remove_threadsafe_function_ref_counter(&mut self, id: usize) {
    let mut counters = self.tsfn_ref_counters.borrow_mut();
    let index = counters.iter().position(|(i, _)| *i == id).unwrap();
    counters.remove(index);
  }
}

pub fn init<P: NapiPermissions + 'static>(unstable: bool) -> Extension {
  Extension::builder(env!("CARGO_PKG_NAME"))
    .ops(vec![op_napi_open::decl::<P>()])
    .event_loop_middleware(|op_state_rc, cx| {
      // `work` can call back into the runtime. It can also schedule an async task
      // but we don't know that now. We need to make the runtime re-poll to make
      // sure no pending NAPI tasks exist.
      let mut maybe_scheduling = false;

      {
        let mut op_state = op_state_rc.borrow_mut();
        let napi_state = op_state.borrow_mut::<NapiState>();

        while let Poll::Ready(Some(async_work_fut)) =
          napi_state.async_work_receiver.poll_next_unpin(cx)
        {
          napi_state.pending_async_work.push(async_work_fut);
        }

        if napi_state.active_threadsafe_functions > 0 {
          maybe_scheduling = true;
        }

        let tsfn_ref_counters = napi_state.tsfn_ref_counters.borrow().clone();
        for (_id, counter) in tsfn_ref_counters.iter() {
          if counter.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            maybe_scheduling = true;
            break;
          }
        }
      }

      loop {
        let maybe_work = {
          let mut op_state = op_state_rc.borrow_mut();
          let napi_state = op_state.borrow_mut::<NapiState>();
          napi_state.pending_async_work.pop()
        };

        if let Some(work) = maybe_work {
          work();
          maybe_scheduling = true;
        } else {
          break;
        }
      }

      maybe_scheduling
    })
    .state(move |state| {
      let (async_work_sender, async_work_receiver) =
        mpsc::unbounded::<PendingNapiAsyncWork>();
      let (threadsafe_function_sender, threadsafe_function_receiver) =
        mpsc::unbounded::<ThreadSafeFunctionStatus>();
      state.put(NapiState {
        pending_async_work: Vec::new(),
        async_work_sender,
        async_work_receiver,
        threadsafe_function_sender,
        threadsafe_function_receiver,
        active_threadsafe_functions: 0,
        env_cleanup_hooks: Rc::new(RefCell::new(vec![])),
        tsfn_ref_counters: Rc::new(RefCell::new(vec![])),
      });
      state.put(Unstable(unstable));
      Ok(())
    })
    .build()
}

pub trait NapiPermissions {
  fn check(&mut self, path: Option<&Path>)
    -> std::result::Result<(), AnyError>;
}

pub struct Unstable(pub bool);

fn check_unstable(state: &OpState) {
  let unstable = state.borrow::<Unstable>();

  if !unstable.0 {
    eprintln!("Unstable API 'node-api'. The --unstable flag must be provided.");
    std::process::exit(70);
  }
}

#[op(v8)]
fn op_napi_open<NP, 'scope>(
  scope: &mut v8::HandleScope<'scope>,
  op_state: &mut OpState,
  path: String,
) -> std::result::Result<serde_v8::Value<'scope>, AnyError>
where
  NP: NapiPermissions + 'static,
{
  check_unstable(op_state);
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
      napi_state.async_work_sender.clone(),
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

  MODULE.with(|cell| {
    let slot = *cell.borrow();
    let obj = match slot {
      Some(nm) => {
        // SAFETY: napi_register_module guarantees that `nm` is valid.
        let nm = unsafe { &*nm };
        assert_eq!(nm.nm_version, 1);
        // SAFETY: we are going blind, calling the register function on the other side.
        let exports = unsafe {
          (nm.nm_register_func)(
            env_ptr,
            std::mem::transmute::<v8::Local<v8::Value>, napi_value>(
              exports.into(),
            ),
          )
        };

        // SAFETY: v8::Local is a pointer to a value and napi_value is also a pointer
        // to a value, they have the same layout
        let exports = unsafe {
          std::mem::transmute::<napi_value, v8::Local<v8::Value>>(exports)
        };
        Ok(serde_v8::Value { v8_value: exports })
      }
      None => {
        // Initializer callback.
        // SAFETY: we are going blind, calling the register function on the other side.
        unsafe {
          let init = library
            .get::<unsafe extern "C" fn(
              env: napi_env,
              exports: napi_value,
            ) -> napi_value>(b"napi_register_module_v1")
            .expect("napi_register_module_v1 not found");
          init(
            env_ptr,
            std::mem::transmute::<v8::Local<v8::Value>, napi_value>(
              exports.into(),
            ),
          )
        };

        Ok(serde_v8::Value {
          v8_value: exports.into(),
        })
      }
    };
    // NAPI addons can't be unloaded, so we're going to "forget" the library
    // object so it lives till the program exit.
    std::mem::forget(library);
    obj
  })
}
