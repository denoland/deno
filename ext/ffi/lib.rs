// Copyright 2018-2026 the Deno authors. MIT license.

use std::mem::size_of;
use std::os::raw::c_char;
use std::os::raw::c_short;

mod call;
mod callback;
mod dlfcn;
mod ir;
mod repr;
mod r#static;
mod symbol;
mod turbocall;

pub use call::CallError;
use call::op_ffi_call_nonblocking;
use call::op_ffi_call_ptr;
use call::op_ffi_call_ptr_nonblocking;
pub use callback::CallbackError;
use callback::UnsafeCallbackResource;
use callback::op_ffi_unsafe_callback_close;
use callback::op_ffi_unsafe_callback_create;
use callback::op_ffi_unsafe_callback_ref;
use deno_core::OpState;
pub use denort_helper::DenoRtNativeAddonLoader;
pub use denort_helper::DenoRtNativeAddonLoaderRc;
pub use dlfcn::DlfcnError;
use dlfcn::DynamicLibraryResource;
use dlfcn::ForeignFunction;
use dlfcn::op_ffi_load;
pub use ir::IRError;
pub use repr::ReprError;
use repr::*;
pub use r#static::StaticError;
use r#static::op_ffi_get_static;
use symbol::NativeType;
use symbol::Symbol;
use turbocall::op_ffi_get_turbocall_target;

#[cfg(not(target_pointer_width = "64"))]
compile_error!("platform not supported");

const _: () = {
  assert!(size_of::<c_char>() == 1);
  assert!(size_of::<c_short>() == 2);
  assert!(size_of::<*const ()>() == 8);
};

pub const UNSTABLE_FEATURE_NAME: &str = "ffi";

/// Native state a runtime's managed native calls can still use after the
/// runtime is dropped: callback closures, which own their libffi `Cif` and
/// trampoline, and loaded libraries, which own the code being executed.
pub struct NativeCallKeepAlive {
  // Fields drop in order: libraries first, so an unload destructor that calls
  // a stored callback still reaches a live trampoline (which then refuses to
  // enter the disposed isolate).
  #[allow(dead_code, reason = "held only to keep resources alive")]
  libraries: Vec<std::rc::Rc<DynamicLibraryResource>>,
  #[allow(dead_code, reason = "held only to keep resources alive")]
  callbacks: Vec<std::rc::Rc<UnsafeCallbackResource>>,
}

/// Take [`NativeCallKeepAlive`] out of the resource table so the rest of the
/// runtime can be dropped while blocking FFI calls finish. Close the runtime's
/// task spawner first: a callback invoked after that fails to dispatch and
/// returns a zero value through its retained `Cif`, with its leaked
/// `CallbackInfo` still valid.
pub fn take_native_call_keepalive(state: &mut OpState) -> NativeCallKeepAlive {
  let rids = state
    .resource_table
    .names()
    .map(|(rid, _)| rid)
    .collect::<Vec<_>>();
  let mut libraries = Vec::new();
  let mut callbacks = Vec::new();
  for rid in rids {
    if let Ok(callback) =
      state.resource_table.take::<UnsafeCallbackResource>(rid)
    {
      callbacks.push(callback);
    } else if let Ok(library) =
      state.resource_table.take::<DynamicLibraryResource>(rid)
    {
      libraries.push(library);
    }
  }
  NativeCallKeepAlive {
    libraries,
    callbacks,
  }
}

deno_core::extension!(deno_ffi,
  deps = [ deno_web ],
  ops = [
    op_ffi_load,
    op_ffi_get_static,
    op_ffi_call_nonblocking,
    op_ffi_call_ptr,
    op_ffi_call_ptr_nonblocking,
    op_ffi_ptr_create,
    op_ffi_ptr_equals,
    op_ffi_ptr_of,
    op_ffi_ptr_of_exact,
    op_ffi_ptr_offset,
    op_ffi_ptr_value,
    op_ffi_get_buf,
    op_ffi_buf_copy_into,
    op_ffi_cstr_read,
    op_ffi_read_bool,
    op_ffi_read_u8,
    op_ffi_read_i8,
    op_ffi_read_u16,
    op_ffi_read_i16,
    op_ffi_read_u32,
    op_ffi_read_i32,
    op_ffi_read_u64,
    op_ffi_read_i64,
    op_ffi_read_f32,
    op_ffi_read_f64,
    op_ffi_read_ptr,
    op_ffi_unsafe_callback_create,
    op_ffi_unsafe_callback_close,
    op_ffi_unsafe_callback_ref,
    op_ffi_get_turbocall_target,
  ],
  lazy_loaded_js = [ "00_ffi.js" ],
  options = {
    deno_rt_native_addon_loader: Option<DenoRtNativeAddonLoaderRc>,
  },
  state = |state, options| {
    if let Some(loader) = options.deno_rt_native_addon_loader {
      state.put(loader);
    }
  },
);
