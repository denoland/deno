// Copyright 2018-2025 the Deno authors. MIT license.

use crate::op2;

#[op2(fast, no_side_effects)]
pub fn op_is_any_array_buffer(value: &v8::Value) -> bool {
  value.is_array_buffer() || value.is_shared_array_buffer()
}

#[op2(fast, no_side_effects)]
pub fn op_is_arguments_object(value: &v8::Value) -> bool {
  value.is_arguments_object()
}

#[op2(fast, no_side_effects)]
pub fn op_is_array_buffer(value: &v8::Value) -> bool {
  value.is_array_buffer()
}

#[op2(fast, no_side_effects)]
pub fn op_is_array_buffer_view(value: &v8::Value) -> bool {
  value.is_array_buffer_view()
}

#[op2(fast, no_side_effects)]
pub fn op_is_async_function(value: &v8::Value) -> bool {
  value.is_async_function()
}

#[op2(fast, no_side_effects)]
pub fn op_is_big_int_object(value: &v8::Value) -> bool {
  value.is_big_int_object()
}

#[op2(fast, no_side_effects)]
pub fn op_is_boolean_object(value: &v8::Value) -> bool {
  value.is_boolean_object()
}

#[op2(fast, no_side_effects)]
pub fn op_is_boxed_primitive(value: &v8::Value) -> bool {
  value.is_boolean_object()
    || value.is_string_object()
    || value.is_number_object()
    || value.is_symbol_object()
    || value.is_big_int_object()
}

#[op2(fast, no_side_effects)]
pub fn op_is_data_view(value: &v8::Value) -> bool {
  value.is_data_view()
}

#[op2(fast, no_side_effects)]
pub fn op_is_date(value: &v8::Value) -> bool {
  value.is_date()
}

#[op2(fast, no_side_effects)]
pub fn op_is_generator_function(value: &v8::Value) -> bool {
  value.is_generator_function()
}

#[op2(fast, no_side_effects)]
pub fn op_is_generator_object(value: &v8::Value) -> bool {
  value.is_generator_object()
}

#[op2(fast, no_side_effects)]
pub fn op_is_map(value: &v8::Value) -> bool {
  value.is_map()
}

#[op2(fast, no_side_effects)]
pub fn op_is_map_iterator(value: &v8::Value) -> bool {
  value.is_map_iterator()
}

#[op2(fast, no_side_effects)]
pub fn op_is_module_namespace_object(value: &v8::Value) -> bool {
  value.is_module_namespace_object()
}

#[op2(fast, reentrant)] // may be invoked by `format_exception_cb`
pub fn op_is_native_error(value: &v8::Value) -> bool {
  value.is_native_error()
}

#[op2(fast, no_side_effects)]
pub fn op_is_number_object(value: &v8::Value) -> bool {
  value.is_number_object()
}

#[op2(fast, no_side_effects)]
pub fn op_is_promise(value: &v8::Value) -> bool {
  value.is_promise()
}

#[op2(fast, no_side_effects)]
pub fn op_is_proxy(value: &v8::Value) -> bool {
  value.is_proxy()
}

#[op2(fast, no_side_effects)]
pub fn op_is_reg_exp(value: &v8::Value) -> bool {
  value.is_reg_exp()
}

#[op2(fast, no_side_effects)]
pub fn op_is_set(value: &v8::Value) -> bool {
  value.is_set()
}

#[op2(fast, no_side_effects)]
pub fn op_is_set_iterator(value: &v8::Value) -> bool {
  value.is_set_iterator()
}

#[op2(fast, no_side_effects)]
pub fn op_is_shared_array_buffer(value: &v8::Value) -> bool {
  value.is_shared_array_buffer()
}

#[op2(fast, no_side_effects)]
pub fn op_is_string_object(value: &v8::Value) -> bool {
  value.is_string_object()
}

#[op2(fast, no_side_effects)]
pub fn op_is_symbol_object(value: &v8::Value) -> bool {
  value.is_symbol_object()
}

#[op2(fast, no_side_effects)]
pub fn op_is_typed_array(value: &v8::Value) -> bool {
  value.is_typed_array()
}

#[op2(fast, no_side_effects)]
pub fn op_is_weak_map(value: &v8::Value) -> bool {
  value.is_weak_map()
}

#[op2(fast, no_side_effects)]
pub fn op_is_weak_set(value: &v8::Value) -> bool {
  value.is_weak_set()
}
