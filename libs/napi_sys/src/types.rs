// Copyright 2018-2026 the Deno authors. MIT license.

// Forked from napi-sys 2.2.2 types.rs — all feature gates removed.

#![allow(non_upper_case_globals, reason = "native code")]
#![allow(non_camel_case_types, reason = "native code")]
#![allow(non_snake_case, reason = "native code")]
#![allow(dead_code, reason = "native code")]

use std::os::raw::c_char;
use std::os::raw::c_int;
use std::os::raw::c_uint;
use std::os::raw::c_void;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_env__ {
  _unused: [u8; 0],
}
pub type napi_env = *mut napi_env__;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_value__ {
  _unused: [u8; 0],
}
pub type napi_value = *mut napi_value__;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_ref__ {
  _unused: [u8; 0],
}
pub type napi_ref = *mut napi_ref__;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_handle_scope__ {
  _unused: [u8; 0],
}
pub type napi_handle_scope = *mut napi_handle_scope__;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_escapable_handle_scope__ {
  _unused: [u8; 0],
}
pub type napi_escapable_handle_scope = *mut napi_escapable_handle_scope__;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_callback_info__ {
  _unused: [u8; 0],
}
pub type napi_callback_info = *mut napi_callback_info__;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_deferred__ {
  _unused: [u8; 0],
}
pub type napi_deferred = *mut napi_deferred__;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct uv_loop_s {
  _unused: [u8; 0],
}

pub type napi_property_attributes = i32;

pub mod PropertyAttributes {
  use super::napi_property_attributes;

  pub const default: napi_property_attributes = 0;
  pub const writable: napi_property_attributes = 1 << 0;
  pub const enumerable: napi_property_attributes = 1 << 1;
  pub const configurable: napi_property_attributes = 1 << 2;
  pub const static_: napi_property_attributes = 1 << 10;
}

pub type napi_valuetype = i32;

pub mod ValueType {
  pub const napi_undefined: i32 = 0;
  pub const napi_null: i32 = 1;
  pub const napi_boolean: i32 = 2;
  pub const napi_number: i32 = 3;
  pub const napi_string: i32 = 4;
  pub const napi_symbol: i32 = 5;
  pub const napi_object: i32 = 6;
  pub const napi_function: i32 = 7;
  pub const napi_external: i32 = 8;
  pub const napi_bigint: i32 = 9;
}

pub type napi_typedarray_type = i32;

pub mod TypedarrayType {
  pub const int8_array: i32 = 0;
  pub const uint8_array: i32 = 1;
  pub const uint8_clamped_array: i32 = 2;
  pub const int16_array: i32 = 3;
  pub const uint16_array: i32 = 4;
  pub const int32_array: i32 = 5;
  pub const uint32_array: i32 = 6;
  pub const float32_array: i32 = 7;
  pub const float64_array: i32 = 8;
  pub const bigint64_array: i32 = 9;
  pub const biguint64_array: i32 = 10;
}

pub type napi_status = i32;

pub mod Status {
  pub const napi_ok: i32 = 0;
  pub const napi_invalid_arg: i32 = 1;
  pub const napi_object_expected: i32 = 2;
  pub const napi_string_expected: i32 = 3;
  pub const napi_name_expected: i32 = 4;
  pub const napi_function_expected: i32 = 5;
  pub const napi_number_expected: i32 = 6;
  pub const napi_boolean_expected: i32 = 7;
  pub const napi_array_expected: i32 = 8;
  pub const napi_generic_failure: i32 = 9;
  pub const napi_pending_exception: i32 = 10;
  pub const napi_cancelled: i32 = 11;
  pub const napi_escape_called_twice: i32 = 12;
  pub const napi_handle_scope_mismatch: i32 = 13;
  pub const napi_callback_scope_mismatch: i32 = 14;
  pub const napi_queue_full: i32 = 15;
  pub const napi_closing: i32 = 16;
  pub const napi_bigint_expected: i32 = 17;
  pub const napi_date_expected: i32 = 18;
  pub const napi_arraybuffer_expected: i32 = 19;
  pub const napi_detachable_arraybuffer_expected: i32 = 20;
  pub const napi_would_deadlock: i32 = 21;
}

pub type napi_callback = Option<
  unsafe extern "C" fn(env: napi_env, info: napi_callback_info) -> napi_value,
>;
pub type napi_finalize = Option<
  unsafe extern "C" fn(
    env: napi_env,
    finalize_data: *mut c_void,
    finalize_hint: *mut c_void,
  ),
>;

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
#[derive(Copy, Clone)]
pub struct napi_extended_error_info {
  pub error_message: *const c_char,
  pub engine_reserved: *mut c_void,
  pub engine_error_code: u32,
  pub error_code: napi_status,
}

pub type napi_key_collection_mode = i32;

pub mod KeyCollectionMode {
  pub use super::napi_key_collection_mode;
  pub const include_prototypes: napi_key_collection_mode = 0;
  pub const own_only: napi_key_collection_mode = 1;
}

pub type napi_key_filter = i32;

pub mod KeyFilter {
  use super::napi_key_filter;

  pub const all_properties: napi_key_filter = 0;
  pub const writable: napi_key_filter = 1;
  pub const enumerable: napi_key_filter = 1 << 1;
  pub const configurable: napi_key_filter = 1 << 2;
  pub const skip_strings: napi_key_filter = 1 << 3;
  pub const skip_symbols: napi_key_filter = 1 << 4;
}

pub type napi_key_conversion = i32;

pub mod KeyConversion {
  use super::napi_key_conversion;

  pub const keep_numbers: napi_key_conversion = 0;
  pub const numbers_to_strings: napi_key_conversion = 1;
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct napi_async_cleanup_hook_handle__ {
  _unused: [u8; 0],
}
pub type napi_async_cleanup_hook_handle = *mut napi_async_cleanup_hook_handle__;
pub type napi_async_cleanup_hook = Option<
  unsafe extern "C" fn(
    handle: napi_async_cleanup_hook_handle,
    data: *mut c_void,
  ),
>;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_callback_scope__ {
  _unused: [u8; 0],
}
pub type napi_callback_scope = *mut napi_callback_scope__;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_async_context__ {
  _unused: [u8; 0],
}
pub type napi_async_context = *mut napi_async_context__;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_async_work__ {
  _unused: [u8; 0],
}
pub type napi_async_work = *mut napi_async_work__;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_threadsafe_function__ {
  _unused: [u8; 0],
}
pub type napi_threadsafe_function = *mut napi_threadsafe_function__;

pub type napi_threadsafe_function_release_mode = i32;

pub mod ThreadsafeFunctionReleaseMode {
  use super::napi_threadsafe_function_release_mode;
  pub const release: napi_threadsafe_function_release_mode = 0;
  pub const abort: napi_threadsafe_function_release_mode = 1;
}

pub type napi_threadsafe_function_call_mode = i32;

pub mod ThreadsafeFunctionCallMode {
  use super::napi_threadsafe_function_call_mode;

  pub const nonblocking: napi_threadsafe_function_call_mode = 0;
  pub const blocking: napi_threadsafe_function_call_mode = 1;
}

pub type napi_async_execute_callback =
  Option<unsafe extern "C" fn(env: napi_env, data: *mut c_void)>;
pub type napi_async_complete_callback = Option<
  unsafe extern "C" fn(env: napi_env, status: napi_status, data: *mut c_void),
>;

pub type napi_threadsafe_function_call_js = Option<
  unsafe extern "C" fn(
    env: napi_env,
    js_callback: napi_value,
    context: *mut c_void,
    data: *mut c_void,
  ),
>;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_node_version {
  pub major: u32,
  pub minor: u32,
  pub patch: u32,
  pub release: *const c_char,
}

pub type napi_addon_register_func = Option<
  unsafe extern "C" fn(env: napi_env, exports: napi_value) -> napi_value,
>;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct napi_module {
  pub nm_version: c_int,
  pub nm_flags: c_uint,
  pub nm_filename: *const c_char,
  pub nm_register_func: napi_addon_register_func,
  pub nm_modname: *const c_char,
  pub nm_priv: *mut c_void,
  pub reserved: [*mut c_void; 4usize],
}
