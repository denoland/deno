// Copyright 2018-2025 the Deno authors. MIT license.

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
use callback::op_ffi_unsafe_callback_close;
use callback::op_ffi_unsafe_callback_create;
use callback::op_ffi_unsafe_callback_ref;
pub use denort_helper::DenoRtNativeAddonLoader;
pub use denort_helper::DenoRtNativeAddonLoaderRc;
pub use dlfcn::DlfcnError;
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
  esm = [ "00_ffi.js" ],
  options = {
    deno_rt_native_addon_loader: Option<DenoRtNativeAddonLoaderRc>,
  },
  state = |state, options| {
    if let Some(loader) = options.deno_rt_native_addon_loader {
      state.put(loader);
    }
  },
);
