// Copyright 2018-2026 the Deno authors. MIT license.

// Forked from napi-sys 2.2.2 functions.rs — all feature gates removed,
// all napi versions flattened into a single block.
// New symbols can be added here directly without upgrading an external crate.

#![allow(clippy::too_many_arguments, reason = "not code we control")]

use std::os::raw::c_char;
use std::os::raw::c_int;
use std::os::raw::c_void;

use super::types::*;

generate!(
  extern "C" {
    // === napi1 ===
    fn napi_get_last_error_info(
      env: napi_env,
      result: *mut *const napi_extended_error_info,
    ) -> napi_status;
    fn napi_get_undefined(
      env: napi_env,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_get_null(env: napi_env, result: *mut napi_value) -> napi_status;
    fn napi_get_global(env: napi_env, result: *mut napi_value) -> napi_status;
    fn napi_get_boolean(
      env: napi_env,
      value: bool,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_object(
      env: napi_env,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_array(env: napi_env, result: *mut napi_value)
    -> napi_status;
    fn napi_create_array_with_length(
      env: napi_env,
      length: usize,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_double(
      env: napi_env,
      value: f64,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_int32(
      env: napi_env,
      value: i32,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_uint32(
      env: napi_env,
      value: u32,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_int64(
      env: napi_env,
      value: i64,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_string_latin1(
      env: napi_env,
      str_: *const c_char,
      length: usize,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_string_utf8(
      env: napi_env,
      str_: *const c_char,
      length: usize,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_string_utf16(
      env: napi_env,
      str_: *const u16,
      length: usize,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_symbol(
      env: napi_env,
      description: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_function(
      env: napi_env,
      utf8name: *const c_char,
      length: usize,
      cb: napi_callback,
      data: *mut c_void,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_error(
      env: napi_env,
      code: napi_value,
      msg: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_type_error(
      env: napi_env,
      code: napi_value,
      msg: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_range_error(
      env: napi_env,
      code: napi_value,
      msg: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_typeof(
      env: napi_env,
      value: napi_value,
      result: *mut napi_valuetype,
    ) -> napi_status;
    fn napi_get_value_double(
      env: napi_env,
      value: napi_value,
      result: *mut f64,
    ) -> napi_status;
    fn napi_get_value_int32(
      env: napi_env,
      value: napi_value,
      result: *mut i32,
    ) -> napi_status;
    fn napi_get_value_uint32(
      env: napi_env,
      value: napi_value,
      result: *mut u32,
    ) -> napi_status;
    fn napi_get_value_int64(
      env: napi_env,
      value: napi_value,
      result: *mut i64,
    ) -> napi_status;
    fn napi_get_value_bool(
      env: napi_env,
      value: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_get_value_string_latin1(
      env: napi_env,
      value: napi_value,
      buf: *mut c_char,
      bufsize: usize,
      result: *mut usize,
    ) -> napi_status;
    fn napi_get_value_string_utf8(
      env: napi_env,
      value: napi_value,
      buf: *mut c_char,
      bufsize: usize,
      result: *mut usize,
    ) -> napi_status;
    fn napi_get_value_string_utf16(
      env: napi_env,
      value: napi_value,
      buf: *mut u16,
      bufsize: usize,
      result: *mut usize,
    ) -> napi_status;
    fn napi_coerce_to_bool(
      env: napi_env,
      value: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_coerce_to_number(
      env: napi_env,
      value: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_coerce_to_object(
      env: napi_env,
      value: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_coerce_to_string(
      env: napi_env,
      value: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_get_prototype(
      env: napi_env,
      object: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_get_property_names(
      env: napi_env,
      object: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_set_property(
      env: napi_env,
      object: napi_value,
      key: napi_value,
      value: napi_value,
    ) -> napi_status;
    fn napi_has_property(
      env: napi_env,
      object: napi_value,
      key: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_get_property(
      env: napi_env,
      object: napi_value,
      key: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_delete_property(
      env: napi_env,
      object: napi_value,
      key: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_has_own_property(
      env: napi_env,
      object: napi_value,
      key: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_set_named_property(
      env: napi_env,
      object: napi_value,
      utf8name: *const c_char,
      value: napi_value,
    ) -> napi_status;
    fn napi_has_named_property(
      env: napi_env,
      object: napi_value,
      utf8name: *const c_char,
      result: *mut bool,
    ) -> napi_status;
    fn napi_get_named_property(
      env: napi_env,
      object: napi_value,
      utf8name: *const c_char,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_set_element(
      env: napi_env,
      object: napi_value,
      index: u32,
      value: napi_value,
    ) -> napi_status;
    fn napi_has_element(
      env: napi_env,
      object: napi_value,
      index: u32,
      result: *mut bool,
    ) -> napi_status;
    fn napi_get_element(
      env: napi_env,
      object: napi_value,
      index: u32,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_delete_element(
      env: napi_env,
      object: napi_value,
      index: u32,
      result: *mut bool,
    ) -> napi_status;
    fn napi_define_properties(
      env: napi_env,
      object: napi_value,
      property_count: usize,
      properties: *const napi_property_descriptor,
    ) -> napi_status;
    fn napi_is_array(
      env: napi_env,
      value: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_get_array_length(
      env: napi_env,
      value: napi_value,
      result: *mut u32,
    ) -> napi_status;
    fn napi_strict_equals(
      env: napi_env,
      lhs: napi_value,
      rhs: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_call_function(
      env: napi_env,
      recv: napi_value,
      func: napi_value,
      argc: usize,
      argv: *const napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_new_instance(
      env: napi_env,
      constructor: napi_value,
      argc: usize,
      argv: *const napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_instanceof(
      env: napi_env,
      object: napi_value,
      constructor: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_get_cb_info(
      env: napi_env,
      cbinfo: napi_callback_info,
      argc: *mut usize,
      argv: *mut napi_value,
      this_arg: *mut napi_value,
      data: *mut *mut c_void,
    ) -> napi_status;
    fn napi_get_new_target(
      env: napi_env,
      cbinfo: napi_callback_info,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_define_class(
      env: napi_env,
      utf8name: *const c_char,
      length: usize,
      constructor: napi_callback,
      data: *mut c_void,
      property_count: usize,
      properties: *const napi_property_descriptor,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_wrap(
      env: napi_env,
      js_object: napi_value,
      native_object: *mut c_void,
      finalize_cb: napi_finalize,
      finalize_hint: *mut c_void,
      result: *mut napi_ref,
    ) -> napi_status;
    fn napi_unwrap(
      env: napi_env,
      js_object: napi_value,
      result: *mut *mut c_void,
    ) -> napi_status;
    fn napi_remove_wrap(
      env: napi_env,
      js_object: napi_value,
      result: *mut *mut c_void,
    ) -> napi_status;
    fn napi_create_external(
      env: napi_env,
      data: *mut c_void,
      finalize_cb: napi_finalize,
      finalize_hint: *mut c_void,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_get_value_external(
      env: napi_env,
      value: napi_value,
      result: *mut *mut c_void,
    ) -> napi_status;
    fn napi_create_reference(
      env: napi_env,
      value: napi_value,
      initial_refcount: u32,
      result: *mut napi_ref,
    ) -> napi_status;
    fn napi_delete_reference(env: napi_env, ref_: napi_ref) -> napi_status;
    fn napi_reference_ref(
      env: napi_env,
      ref_: napi_ref,
      result: *mut u32,
    ) -> napi_status;
    fn napi_reference_unref(
      env: napi_env,
      ref_: napi_ref,
      result: *mut u32,
    ) -> napi_status;
    fn napi_get_reference_value(
      env: napi_env,
      ref_: napi_ref,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_open_handle_scope(
      env: napi_env,
      result: *mut napi_handle_scope,
    ) -> napi_status;
    fn napi_close_handle_scope(
      env: napi_env,
      scope: napi_handle_scope,
    ) -> napi_status;
    fn napi_open_escapable_handle_scope(
      env: napi_env,
      result: *mut napi_escapable_handle_scope,
    ) -> napi_status;
    fn napi_close_escapable_handle_scope(
      env: napi_env,
      scope: napi_escapable_handle_scope,
    ) -> napi_status;
    fn napi_escape_handle(
      env: napi_env,
      scope: napi_escapable_handle_scope,
      escapee: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_throw(env: napi_env, error: napi_value) -> napi_status;
    fn napi_throw_error(
      env: napi_env,
      code: *const c_char,
      msg: *const c_char,
    ) -> napi_status;
    fn napi_throw_type_error(
      env: napi_env,
      code: *const c_char,
      msg: *const c_char,
    ) -> napi_status;
    fn napi_throw_range_error(
      env: napi_env,
      code: *const c_char,
      msg: *const c_char,
    ) -> napi_status;
    fn napi_is_error(
      env: napi_env,
      value: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_is_exception_pending(
      env: napi_env,
      result: *mut bool,
    ) -> napi_status;
    fn napi_get_and_clear_last_exception(
      env: napi_env,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_is_arraybuffer(
      env: napi_env,
      value: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_create_arraybuffer(
      env: napi_env,
      byte_length: usize,
      data: *mut *mut c_void,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_external_arraybuffer(
      env: napi_env,
      external_data: *mut c_void,
      byte_length: usize,
      finalize_cb: napi_finalize,
      finalize_hint: *mut c_void,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_get_arraybuffer_info(
      env: napi_env,
      arraybuffer: napi_value,
      data: *mut *mut c_void,
      byte_length: *mut usize,
    ) -> napi_status;
    fn napi_is_typedarray(
      env: napi_env,
      value: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_create_typedarray(
      env: napi_env,
      type_: napi_typedarray_type,
      length: usize,
      arraybuffer: napi_value,
      byte_offset: usize,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_get_typedarray_info(
      env: napi_env,
      typedarray: napi_value,
      type_: *mut napi_typedarray_type,
      length: *mut usize,
      data: *mut *mut c_void,
      arraybuffer: *mut napi_value,
      byte_offset: *mut usize,
    ) -> napi_status;
    fn napi_create_dataview(
      env: napi_env,
      length: usize,
      arraybuffer: napi_value,
      byte_offset: usize,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_is_dataview(
      env: napi_env,
      value: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_get_dataview_info(
      env: napi_env,
      dataview: napi_value,
      bytelength: *mut usize,
      data: *mut *mut c_void,
      arraybuffer: *mut napi_value,
      byte_offset: *mut usize,
    ) -> napi_status;
    fn napi_get_version(env: napi_env, result: *mut u32) -> napi_status;
    fn napi_create_promise(
      env: napi_env,
      deferred: *mut napi_deferred,
      promise: *mut napi_value,
    ) -> napi_status;
    fn napi_resolve_deferred(
      env: napi_env,
      deferred: napi_deferred,
      resolution: napi_value,
    ) -> napi_status;
    fn napi_reject_deferred(
      env: napi_env,
      deferred: napi_deferred,
      rejection: napi_value,
    ) -> napi_status;
    fn napi_is_promise(
      env: napi_env,
      value: napi_value,
      is_promise: *mut bool,
    ) -> napi_status;
    fn napi_run_script(
      env: napi_env,
      script: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_adjust_external_memory(
      env: napi_env,
      change_in_bytes: i64,
      adjusted_value: *mut i64,
    ) -> napi_status;
    fn napi_module_register(mod_: *mut napi_module);
    fn napi_fatal_error(
      location: *const c_char,
      location_len: usize,
      message: *const c_char,
      message_len: usize,
    );
    fn napi_async_init(
      env: napi_env,
      async_resource: napi_value,
      async_resource_name: napi_value,
      result: *mut napi_async_context,
    ) -> napi_status;
    fn napi_async_destroy(
      env: napi_env,
      async_context: napi_async_context,
    ) -> napi_status;
    fn napi_make_callback(
      env: napi_env,
      async_context: napi_async_context,
      recv: napi_value,
      func: napi_value,
      argc: usize,
      argv: *const napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_buffer(
      env: napi_env,
      length: usize,
      data: *mut *mut c_void,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_external_buffer(
      env: napi_env,
      length: usize,
      data: *mut c_void,
      finalize_cb: napi_finalize,
      finalize_hint: *mut c_void,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_buffer_copy(
      env: napi_env,
      length: usize,
      data: *const c_void,
      result_data: *mut *mut c_void,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_is_buffer(
      env: napi_env,
      value: napi_value,
      result: *mut bool,
    ) -> napi_status;
    fn napi_get_buffer_info(
      env: napi_env,
      value: napi_value,
      data: *mut *mut c_void,
      length: *mut usize,
    ) -> napi_status;
    fn napi_create_async_work(
      env: napi_env,
      async_resource: napi_value,
      async_resource_name: napi_value,
      execute: napi_async_execute_callback,
      complete: napi_async_complete_callback,
      data: *mut c_void,
      result: *mut napi_async_work,
    ) -> napi_status;
    fn napi_delete_async_work(
      env: napi_env,
      work: napi_async_work,
    ) -> napi_status;
    fn napi_queue_async_work(
      env: napi_env,
      work: napi_async_work,
    ) -> napi_status;
    fn napi_cancel_async_work(
      env: napi_env,
      work: napi_async_work,
    ) -> napi_status;
    fn napi_get_node_version(
      env: napi_env,
      version: *mut *const napi_node_version,
    ) -> napi_status;

    // === napi2 ===
    fn napi_get_uv_event_loop(
      env: napi_env,
      loop_: *mut *mut uv_loop_s,
    ) -> napi_status;

    // === napi3 ===
    fn napi_fatal_exception(env: napi_env, err: napi_value) -> napi_status;
    fn napi_add_env_cleanup_hook(
      env: napi_env,
      fun: Option<unsafe extern "C" fn(arg: *mut c_void)>,
      arg: *mut c_void,
    ) -> napi_status;
    fn napi_remove_env_cleanup_hook(
      env: napi_env,
      fun: Option<unsafe extern "C" fn(arg: *mut c_void)>,
      arg: *mut c_void,
    ) -> napi_status;
    fn napi_open_callback_scope(
      env: napi_env,
      resource_object: napi_value,
      context: napi_async_context,
      result: *mut napi_callback_scope,
    ) -> napi_status;
    fn napi_close_callback_scope(
      env: napi_env,
      scope: napi_callback_scope,
    ) -> napi_status;

    // === napi4 ===
    fn napi_create_threadsafe_function(
      env: napi_env,
      func: napi_value,
      async_resource: napi_value,
      async_resource_name: napi_value,
      max_queue_size: usize,
      initial_thread_count: usize,
      thread_finalize_data: *mut c_void,
      thread_finalize_cb: napi_finalize,
      context: *mut c_void,
      call_js_cb: napi_threadsafe_function_call_js,
      result: *mut napi_threadsafe_function,
    ) -> napi_status;
    fn napi_get_threadsafe_function_context(
      func: napi_threadsafe_function,
      result: *mut *mut c_void,
    ) -> napi_status;
    fn napi_call_threadsafe_function(
      func: napi_threadsafe_function,
      data: *mut c_void,
      is_blocking: napi_threadsafe_function_call_mode,
    ) -> napi_status;
    fn napi_acquire_threadsafe_function(
      func: napi_threadsafe_function,
    ) -> napi_status;
    fn napi_release_threadsafe_function(
      func: napi_threadsafe_function,
      mode: napi_threadsafe_function_release_mode,
    ) -> napi_status;
    fn napi_unref_threadsafe_function(
      env: napi_env,
      func: napi_threadsafe_function,
    ) -> napi_status;
    fn napi_ref_threadsafe_function(
      env: napi_env,
      func: napi_threadsafe_function,
    ) -> napi_status;

    // === napi5 ===
    fn napi_create_date(
      env: napi_env,
      time: f64,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_is_date(
      env: napi_env,
      value: napi_value,
      is_date: *mut bool,
    ) -> napi_status;
    fn napi_get_date_value(
      env: napi_env,
      value: napi_value,
      result: *mut f64,
    ) -> napi_status;
    fn napi_add_finalizer(
      env: napi_env,
      js_object: napi_value,
      native_object: *mut c_void,
      finalize_cb: napi_finalize,
      finalize_hint: *mut c_void,
      result: *mut napi_ref,
    ) -> napi_status;

    // === napi6 ===
    fn napi_create_bigint_int64(
      env: napi_env,
      value: i64,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_bigint_uint64(
      env: napi_env,
      value: u64,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_create_bigint_words(
      env: napi_env,
      sign_bit: c_int,
      word_count: usize,
      words: *const u64,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_get_value_bigint_int64(
      env: napi_env,
      value: napi_value,
      result: *mut i64,
      lossless: *mut bool,
    ) -> napi_status;
    fn napi_get_value_bigint_uint64(
      env: napi_env,
      value: napi_value,
      result: *mut u64,
      lossless: *mut bool,
    ) -> napi_status;
    fn napi_get_value_bigint_words(
      env: napi_env,
      value: napi_value,
      sign_bit: *mut c_int,
      word_count: *mut usize,
      words: *mut u64,
    ) -> napi_status;
    fn napi_get_all_property_names(
      env: napi_env,
      object: napi_value,
      key_mode: napi_key_collection_mode,
      key_filter: napi_key_filter,
      key_conversion: napi_key_conversion,
      result: *mut napi_value,
    ) -> napi_status;
    fn napi_set_instance_data(
      env: napi_env,
      data: *mut c_void,
      finalize_cb: napi_finalize,
      finalize_hint: *mut c_void,
    ) -> napi_status;
    fn napi_get_instance_data(
      env: napi_env,
      data: *mut *mut c_void,
    ) -> napi_status;

    // === napi7 ===
    fn napi_detach_arraybuffer(
      env: napi_env,
      arraybuffer: napi_value,
    ) -> napi_status;
    fn napi_is_detached_arraybuffer(
      env: napi_env,
      value: napi_value,
      result: *mut bool,
    ) -> napi_status;

    // === napi8 ===
    fn napi_add_async_cleanup_hook(
      env: napi_env,
      hook: napi_async_cleanup_hook,
      arg: *mut c_void,
      remove_handle: *mut napi_async_cleanup_hook_handle,
    ) -> napi_status;
    fn napi_remove_async_cleanup_hook(
      remove_handle: napi_async_cleanup_hook_handle,
    ) -> napi_status;
    fn napi_object_freeze(env: napi_env, object: napi_value) -> napi_status;
    fn napi_object_seal(env: napi_env, object: napi_value) -> napi_status;

    // === node_api (experimental in upstream, always available here) ===
    fn node_api_get_module_file_name(
      env: napi_env,
      result: *mut *const c_char,
    ) -> napi_status;
    fn node_api_create_syntax_error(
      env: napi_env,
      code: napi_value,
      msg: napi_value,
      result: *mut napi_value,
    ) -> napi_status;
    fn node_api_throw_syntax_error(
      env: napi_env,
      code: *const c_char,
      msg: *const c_char,
    ) -> napi_status;
    fn node_api_symbol_for(
      env: napi_env,
      utf8description: *const c_char,
      length: usize,
      result: *mut napi_value,
    ) -> napi_status;
    fn node_api_create_property_key_utf16(
      env: napi_env,
      str_: *const u16,
      length: usize,
      result: *mut napi_value,
    ) -> napi_status;
    fn node_api_create_property_key_latin1(
      env: napi_env,
      str_: *const c_char,
      length: usize,
      result: *mut napi_value,
    ) -> napi_status;
    fn node_api_create_property_key_utf8(
      env: napi_env,
      str_: *const c_char,
      length: usize,
      result: *mut napi_value,
    ) -> napi_status;
    fn node_api_create_object_with_properties(
      env: napi_env,
      prototype_or_null: napi_value,
      property_names: *const napi_value,
      property_values: *const napi_value,
      property_count: usize,
      result: *mut napi_value,
    ) -> napi_status;
    fn node_api_create_object_with_named_properties(
      env: napi_env,
      result: *mut napi_value,
      property_count: usize,
      property_names: *const *const c_char,
      property_values: *const napi_value,
    ) -> napi_status;
  }
);
