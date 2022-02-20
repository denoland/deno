#![allow(unused_mut)]
pub mod function;
pub mod napi_add_env_cleanup_hook;
pub mod napi_add_finalizer;
pub mod napi_adjust_external_memory;
pub mod napi_call_function;
pub mod napi_call_threadsafe_function;
pub mod napi_cancel_async_work;
pub mod napi_close_escapable_handle_scope;
pub mod napi_close_handle_scope;
pub mod napi_coerce_to_bool;
pub mod napi_coerce_to_number;
pub mod napi_coerce_to_object;
pub mod napi_coerce_to_string;
pub mod napi_create_array_with_length;
pub mod napi_create_arraybuffer;
pub mod napi_create_async_work;
pub mod napi_create_bigint_int64;
pub mod napi_create_bigint_uint64;
pub mod napi_create_bigint_words;
pub mod napi_create_buffer;
pub mod napi_create_buffer_copy;
pub mod napi_create_dataview;
pub mod napi_create_date;
pub mod napi_create_double;
pub mod napi_create_error;
pub mod napi_create_external;
pub mod napi_create_external_arraybuffer;
pub mod napi_create_external_buffer;
pub mod napi_create_function;
pub mod napi_create_int32;
pub mod napi_create_int64;
pub mod napi_create_object;
pub mod napi_create_promise;
pub mod napi_create_range_error;
pub mod napi_create_reference;
pub mod napi_create_string_latin1;
pub mod napi_create_string_utf16;
pub mod napi_create_string_utf8;
pub mod napi_create_symbol;
pub mod napi_create_threadsafe_function;
pub mod napi_create_type_error;
#[allow(non_upper_case_globals)]
pub mod napi_create_typedarray;
pub mod napi_create_uint32;
pub mod napi_define_class;
pub mod napi_define_properties;
pub mod napi_delete_async_work;
pub mod napi_delete_element;
pub mod napi_delete_property;
pub mod napi_delete_reference;
pub mod napi_detach_arraybuffer;
pub mod napi_escape_handle;
pub mod napi_fatal_error;
pub mod napi_fatal_exception;
pub mod napi_get_all_property_names;
pub mod napi_get_and_clear_last_exception;
pub mod napi_get_array_length;
pub mod napi_get_arraybuffer_info;
pub mod napi_get_boolean;
pub mod napi_get_buffer_info;
pub mod napi_get_cb_info;
pub mod napi_get_dataview_info;
pub mod napi_get_date_value;
pub mod napi_get_element;
pub mod napi_get_global;
pub mod napi_get_instance_data;
pub mod napi_get_last_error_info;
pub mod napi_get_named_property;
pub mod napi_get_new_target;
pub mod napi_get_node_version;
pub mod napi_get_null;
pub mod napi_get_property;
pub mod napi_get_property_names;
pub mod napi_get_prototype;
pub mod napi_get_reference_value;
pub mod napi_get_threadsafe_function_context;
pub mod napi_get_typedarray_info;
pub mod napi_get_undefined;
pub mod napi_get_uv_event_loop;
pub mod napi_get_value_bigint_int64;
pub mod napi_get_value_bigint_uint64;
pub mod napi_get_value_bigint_words;
pub mod napi_get_value_bool;
pub mod napi_get_value_double;
pub mod napi_get_value_external;
pub mod napi_get_value_int32;
pub mod napi_get_value_int64;
pub mod napi_get_value_string_latin1;
pub mod napi_get_value_string_utf16;
pub mod napi_get_value_string_utf8;
pub mod napi_get_value_uint32;
pub mod napi_get_version;
pub mod napi_has_element;
pub mod napi_has_named_property;
pub mod napi_has_own_property;
pub mod napi_has_property;
pub mod napi_instanceof;
pub mod napi_is_array;
pub mod napi_is_arraybuffer;
pub mod napi_is_buffer;
pub mod napi_is_dataview;
pub mod napi_is_date;
pub mod napi_is_detached_arraybuffer;
pub mod napi_is_error;
pub mod napi_is_exception_pending;
pub mod napi_is_promise;
pub mod napi_is_typedarray;
pub mod napi_module_register;
pub mod napi_new_instance;
pub mod napi_object_freeze;
pub mod napi_object_seal;
pub mod napi_open_escapable_handle_scope;
pub mod napi_open_handle_scope;
pub mod napi_queue_async_work;
pub mod napi_ref_threadsafe_function;
pub mod napi_reference_ref;
pub mod napi_reference_unref;
pub mod napi_reject_deferred;
pub mod napi_release_threadsafe_function;
pub mod napi_remove_env_cleanup_hook;
pub mod napi_remove_wrap;
pub mod napi_resolve_deferred;
pub mod napi_run_script;
pub mod napi_set_element;
pub mod napi_set_instance_data;
pub mod napi_set_named_property;
pub mod napi_set_property;
pub mod napi_strict_equals;
pub mod napi_throw;
pub mod napi_throw_error;
pub mod napi_throw_range_error;
pub mod napi_throw_type_error;
pub mod napi_typeof;
pub mod napi_unref_threadsafe_function;
pub mod napi_unwrap;
pub mod napi_wrap;
pub mod node_api_create_syntax_error;
pub mod node_api_get_module_file_name;
pub mod node_api_throw_syntax_error;
pub mod util;

use deno_core::v8;
use std::os::raw::c_int;
use std::os::raw::c_void;

pub type uv_async_t = *mut uv_async;
pub type uv_loop_t = *mut c_void;
pub type uv_async_cb = extern "C" fn(handle: uv_async_t);

use deno_core::futures::channel::mpsc;
#[repr(C)]
pub struct uv_async {
  pub data: Option<*mut c_void>,
  callback: uv_async_cb,
  sender: Option<mpsc::UnboundedSender<deno_core::napi::PendingNapiAsyncWork>>,
}

#[no_mangle]
pub extern "C" fn uv_default_loop() -> uv_loop_t {
  std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn uv_async_init(
  _loop: uv_loop_t,
  async_: uv_async_t,
  cb: uv_async_cb,
) -> c_int {
  unsafe {
    (*async_).callback = cb;
  }
  deno_core::napi::ASYNC_WORK_SENDER.with(|sender| {
    unsafe {
      (*async_).sender.replace(sender.borrow().clone().unwrap());
    }
    0
  })
}

#[no_mangle]
pub extern "C" fn uv_async_send(async_: uv_async_t) -> c_int {
  let sender = unsafe { (*async_).sender.as_ref().unwrap() };
  let fut = Box::new(move |scope: &mut v8::HandleScope| {
    drop(scope);
    unsafe { ((*async_).callback)(async_) };
  });

  match sender.unbounded_send(fut) {
    Ok(_) => 0,
    Err(_) => 1,
  }
}
