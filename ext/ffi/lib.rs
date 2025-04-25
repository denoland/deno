// Copyright 2018-2025 the Deno authors. MIT license.

use std::mem::size_of;
use std::os::raw::c_char;
use std::os::raw::c_short;
use std::path::PathBuf;

mod call;
mod callback;
mod dlfcn;
mod ir;
mod repr;
mod r#static;
mod symbol;
mod turbocall;

use call::op_ffi_call_nonblocking;
use call::op_ffi_call_ptr;
use call::op_ffi_call_ptr_nonblocking;
pub use call::CallError;
use callback::op_ffi_unsafe_callback_close;
use callback::op_ffi_unsafe_callback_create;
use callback::op_ffi_unsafe_callback_ref;
pub use callback::CallbackError;
use deno_permissions::PermissionCheckError;
pub use denort_helper::DenoRtNativeAddonLoader;
pub use denort_helper::DenoRtNativeAddonLoaderRc;
use dlfcn::op_ffi_load;
pub use dlfcn::DlfcnError;
use dlfcn::ForeignFunction;
pub use ir::IRError;
use r#static::op_ffi_get_static;
pub use r#static::StaticError;
pub use repr::ReprError;
use repr::*;
use symbol::NativeType;
use symbol::Symbol;

#[cfg(not(target_pointer_width = "64"))]
compile_error!("platform not supported");

const _: () = {
  assert!(size_of::<c_char>() == 1);
  assert!(size_of::<c_short>() == 2);
  assert!(size_of::<*const ()>() == 8);
};

pub const UNSTABLE_FEATURE_NAME: &str = "ffi";

pub trait FfiPermissions {
  fn check_partial_no_path(&mut self) -> Result<(), PermissionCheckError>;
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn check_partial_with_path(
    &mut self,
    path: &str,
  ) -> Result<PathBuf, PermissionCheckError>;
}

impl FfiPermissions for deno_permissions::PermissionsContainer {
  #[inline(always)]
  fn check_partial_no_path(&mut self) -> Result<(), PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_ffi_partial_no_path(self)
  }

  #[inline(always)]
  fn check_partial_with_path(
    &mut self,
    path: &str,
  ) -> Result<PathBuf, PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_ffi_partial_with_path(
      self, path,
    )
  }
}

deno_core::extension!(deno_ffi,
  deps = [ deno_web ],
  parameters = [P: FfiPermissions],
  ops = [
    op_ffi_load<P>,
    op_ffi_get_static,
    op_ffi_call_nonblocking,
    op_ffi_call_ptr<P>,
    op_ffi_call_ptr_nonblocking<P>,
    op_ffi_ptr_create<P>,
    op_ffi_ptr_equals<P>,
    op_ffi_ptr_of<P>,
    op_ffi_ptr_of_exact<P>,
    op_ffi_ptr_offset<P>,
    op_ffi_ptr_value<P>,
    op_ffi_get_buf<P>,
    op_ffi_buf_copy_into<P>,
    op_ffi_cstr_read<P>,
    op_ffi_read_bool<P>,
    op_ffi_read_u8<P>,
    op_ffi_read_i8<P>,
    op_ffi_read_u16<P>,
    op_ffi_read_i16<P>,
    op_ffi_read_u32<P>,
    op_ffi_read_i32<P>,
    op_ffi_read_u64<P>,
    op_ffi_read_i64<P>,
    op_ffi_read_f32<P>,
    op_ffi_read_f64<P>,
    op_ffi_read_ptr<P>,
    op_ffi_unsafe_callback_create<P>,
    op_ffi_unsafe_callback_close,
    op_ffi_unsafe_callback_ref,
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
