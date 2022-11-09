// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

#![allow(non_upper_case_globals)]

use deno_runtime::deno_napi::*;
use v8::BackingStore;
use v8::UniqueRef;

use super::util::get_array_buffer_ptr;
use deno_runtime::deno_napi::function::create_function;
use deno_runtime::deno_napi::function::create_function_template;
use deno_runtime::deno_napi::function::CallbackInfo;
use std::ptr::NonNull;

// Macro to check napi arguments.
// If nullptr, return Err(Error::InvalidArg).
#[macro_export]
macro_rules! check_arg {
  ($ptr: expr) => {
    if $ptr.is_null() {
      return Err(Error::InvalidArg);
    }
  };
}

macro_rules! check_arg_option {
  ($ptr: expr) => {
    if $ptr.is_none() {
      return Err(Error::InvalidArg);
    }
  };
}

/// Returns napi_value that represents a new JavaScript Array.
#[napi_sym::napi_sym]
fn napi_create_array(env: *mut Env, result: *mut napi_value) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  check_arg!(result);

  let value = v8::Array::new(&mut env.scope(), 0);
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_array_with_length(
  env: *mut Env,
  len: i32,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  check_arg!(result);

  let value = v8::Array::new(&mut env.scope(), len);
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_arraybuffer(
  env: *mut Env,
  len: usize,
  data: *mut *mut u8,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  check_arg!(result);

  let value = v8::ArrayBuffer::new(&mut env.scope(), len);
  if !data.is_null() {
    *data = get_array_buffer_ptr(value);
  }

  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_bigint_int64(
  env: *mut Env,
  value: i64,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  check_arg!(result);

  let value = v8::BigInt::new_from_i64(&mut env.scope(), value);
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_bigint_uint64(
  env: *mut Env,
  value: u64,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value: v8::Local<v8::Value> =
    v8::BigInt::new_from_u64(&mut env.scope(), value).into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_bigint_words(
  env: *mut Env,
  sign_bit: bool,
  words: *const u64,
  word_count: usize,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value: v8::Local<v8::Value> = v8::BigInt::new_from_words(
    &mut env.scope(),
    sign_bit,
    std::slice::from_raw_parts(words, word_count),
  )
  .unwrap()
  .into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_buffer(
  env: *mut Env,
  len: usize,
  data: *mut *mut u8,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = v8::ArrayBuffer::new(&mut env.scope(), len);
  if !data.is_null() {
    *data = get_array_buffer_ptr(value);
  }
  let value = v8::Uint8Array::new(&mut env.scope(), value, 0, len).unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_buffer_copy(
  env: *mut Env,
  len: usize,
  data: *mut u8,
  result_data: *mut *mut u8,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = v8::ArrayBuffer::new(&mut env.scope(), len);
  let ptr = get_array_buffer_ptr(value);
  std::ptr::copy(data, ptr, len);
  if !result_data.is_null() {
    *result_data = ptr;
  }
  let value = v8::Uint8Array::new(&mut env.scope(), value, 0, len).unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_coerce_to_bool(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let coerced = value.to_boolean(&mut env.scope());
  let value: v8::Local<v8::Value> = coerced.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_coerce_to_number(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let coerced = value
    .to_number(&mut env.scope())
    .ok_or(Error::NumberExpected)?;
  let value: v8::Local<v8::Value> = coerced.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_coerce_to_object(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let coerced = value.to_object(&mut env.scope()).unwrap();
  let value: v8::Local<v8::Value> = coerced.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_coerce_to_string(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let coerced = value.to_string(&mut env.scope()).unwrap();
  let value: v8::Local<v8::Value> = coerced.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_dataview(
  env: *mut Env,
  len: usize,
  data: *mut *mut u8,
  byte_offset: usize,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  check_arg!(data);
  check_arg!(result);
  let value = v8::ArrayBuffer::new(&mut env.scope(), len);
  if !data.is_null() {
    *data = get_array_buffer_ptr(value);
  }
  let context = &mut env.scope().get_current_context();
  let global = context.global(&mut env.scope());
  let data_view_name = v8::String::new(&mut env.scope(), "DataView").unwrap();
  let data_view = global.get(&mut env.scope(), data_view_name.into()).unwrap();
  let data_view = v8::Local::<v8::Function>::try_from(data_view).unwrap();
  let byte_offset = v8::Number::new(&mut env.scope(), byte_offset as f64);
  let byte_length = v8::Number::new(&mut env.scope(), len as f64);
  let value = data_view
    .new_instance(
      &mut env.scope(),
      &[value.into(), byte_offset.into(), byte_length.into()],
    )
    .unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_date(
  env: *mut Env,
  time: f64,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value: v8::Local<v8::Value> =
    v8::Date::new(&mut env.scope(), time).unwrap().into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_double(
  env: *mut Env,
  value: f64,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value: v8::Local<v8::Value> =
    v8::Number::new(&mut env.scope(), value).into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_error(
  env: *mut Env,
  code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let _code = transmute::<napi_value, v8::Local<v8::Value>>(code);
  let msg = transmute::<napi_value, v8::Local<v8::Value>>(msg);

  let msg = msg.to_string(&mut env.scope()).unwrap();

  let error = v8::Exception::error(&mut env.scope(), msg);
  *result = error.into();

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_external(
  env: *mut Env,
  value: *mut c_void,
  _finalize_cb: napi_finalize,
  _finalize_hint: *mut c_void,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value: v8::Local<v8::Value> =
    v8::External::new(&mut env.scope(), value).into();
  // TODO: finalization
  *result = value.into();
  Ok(())
}

pub type BackingStoreDeleterCallback = unsafe extern "C" fn(
  data: *mut c_void,
  byte_length: usize,
  deleter_data: *mut c_void,
);
extern "C" {
  fn v8__ArrayBuffer__NewBackingStore__with_data(
    data: *mut c_void,
    byte_length: usize,
    deleter: BackingStoreDeleterCallback,
    deleter_data: *mut c_void,
  ) -> *mut BackingStore;
}

pub extern "C" fn backing_store_deleter_callback(
  data: *mut c_void,
  byte_length: usize,
  _deleter_data: *mut c_void,
) {
  let slice_ptr = ptr::slice_from_raw_parts_mut(data as *mut u8, byte_length);
  let b = unsafe { Box::from_raw(slice_ptr) };
  drop(b);
}

#[napi_sym::napi_sym]
fn napi_create_external_arraybuffer(
  env: *mut Env,
  data: *mut c_void,
  byte_length: usize,
  _finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let _slice = std::slice::from_raw_parts(data as *mut u8, byte_length);
  // TODO: finalization
  let store: UniqueRef<BackingStore> =
    transmute(v8__ArrayBuffer__NewBackingStore__with_data(
      data,
      byte_length,
      backing_store_deleter_callback,
      finalize_hint,
    ));

  let ab =
    v8::ArrayBuffer::with_backing_store(&mut env.scope(), &store.make_shared());
  let value: v8::Local<v8::Value> = ab.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_external_buffer(
  env: *mut Env,
  byte_length: isize,
  data: *mut c_void,
  _finalize_cb: napi_finalize,
  _finalize_hint: *mut c_void,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let slice = if byte_length == -1 {
    std::ffi::CStr::from_ptr(data as *const _).to_bytes()
  } else {
    std::slice::from_raw_parts(data as *mut u8, byte_length as usize)
  };
  // TODO: make this not copy the slice
  // TODO: finalization
  let store = v8::ArrayBuffer::new_backing_store_from_boxed_slice(
    slice.to_vec().into_boxed_slice(),
  );
  let ab =
    v8::ArrayBuffer::with_backing_store(&mut env.scope(), &store.make_shared());
  let value =
    v8::Uint8Array::new(&mut env.scope(), ab, 0, slice.len()).unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_function(
  env_ptr: *mut Env,
  name: *const u8,
  length: isize,
  cb: napi_callback,
  cb_info: napi_callback_info,
  result: *mut napi_value,
) -> Result {
  let _: &mut Env = env_ptr.as_mut().ok_or(Error::InvalidArg)?;
  let name = match name.is_null() {
    true => None,
    false => Some(name),
  };
  let name = name.map(|name| {
    if length == -1 {
      std::ffi::CStr::from_ptr(name as *const _).to_str().unwrap()
    } else {
      let name = std::slice::from_raw_parts(name, length as usize);
      // If ends with NULL
      if name[name.len() - 1] == 0 {
        std::str::from_utf8(&name[0..name.len() - 1]).unwrap()
      } else {
        std::str::from_utf8(name).unwrap()
      }
    }
  });

  let function = create_function(env_ptr, name, cb, cb_info);
  let value: v8::Local<v8::Value> = function.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_int32(
  env: *mut Env,
  value: i32,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value: v8::Local<v8::Value> =
    v8::Number::new(&mut env.scope(), value as f64).into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_int64(
  env: *mut Env,
  value: i64,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value: v8::Local<v8::Value> =
    v8::Number::new(&mut env.scope(), value as f64).into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_object(env: *mut Env, result: *mut napi_value) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let object = v8::Object::new(&mut env.scope());
  *result = object.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_promise(
  env: *mut Env,
  deferred: *mut napi_deferred,
  promise_out: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let resolver = v8::PromiseResolver::new(&mut env.scope()).unwrap();
  let mut global = v8::Global::new(&mut env.scope(), resolver);
  let mut global_ptr = global.into_raw();
  let promise = resolver.get_promise(&mut env.scope());
  *deferred = global_ptr.as_mut() as *mut _ as napi_deferred;
  *promise_out = promise.into();

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_range_error(
  env: *mut Env,
  _code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  // let code = transmute::<napi_value, v8::Local<v8::Value>>(code);
  let msg = transmute::<napi_value, v8::Local<v8::Value>>(msg);

  let msg = msg.to_string(&mut env.scope()).unwrap();

  let error = v8::Exception::range_error(&mut env.scope(), msg);
  *result = error.into();

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_reference(
  env: *mut Env,
  value: napi_value,
  _initial_refcount: u32,
  result: *mut napi_ref,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let global = v8::Global::new(&mut env.scope(), value);
  let mut global_ptr = global.into_raw();
  *result = transmute::<NonNull<v8::Value>, napi_ref>(global_ptr);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_string_latin1(
  env: *mut Env,
  string: *const u8,
  length: isize,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let string = if length == -1 {
    std::ffi::CStr::from_ptr(string as *const _)
      .to_str()
      .unwrap()
      .as_bytes()
  } else {
    std::slice::from_raw_parts(string, length as usize)
  };
  match v8::String::new_from_one_byte(
    &mut env.scope(),
    string,
    v8::NewStringType::Normal,
  ) {
    Some(v8str) => {
      let value: v8::Local<v8::Value> = v8str.into();
      *result = value.into();
    }
    None => return Err(Error::GenericFailure),
  }

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_string_utf16(
  env: *mut Env,
  string: *const u16,
  length: usize,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let string = std::slice::from_raw_parts(string, length);
  let v8str = v8::String::new_from_two_byte(
    &mut env.scope(),
    string,
    v8::NewStringType::Normal,
  )
  .unwrap();
  let value: v8::Local<v8::Value> = v8str.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_string_utf8(
  env: *mut Env,
  string: *const u8,
  length: isize,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let string = if length == -1 {
    std::ffi::CStr::from_ptr(string as *const _)
      .to_str()
      .unwrap()
  } else {
    let string = std::slice::from_raw_parts(string, length as usize);
    std::str::from_utf8(string).unwrap()
  };
  let v8str = v8::String::new(&mut env.scope(), string).unwrap();
  let value: v8::Local<v8::Value> = v8str.into();
  *result = value.into();

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_symbol(
  env: *mut Env,
  description: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let description = match description.is_none() {
    true => None,
    false => Some(
      transmute::<napi_value, v8::Local<v8::Value>>(description)
        .to_string(&mut env.scope())
        .unwrap(),
    ),
  };
  let sym = v8::Symbol::new(&mut env.scope(), description);
  *result = sym.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_type_error(
  env: *mut Env,
  _code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  // let code = transmute::<napi_value, v8::Local<v8::Value>>(code);
  let msg = transmute::<napi_value, v8::Local<v8::Value>>(msg);

  let msg = msg.to_string(&mut env.scope()).unwrap();

  let error = v8::Exception::type_error(&mut env.scope(), msg);
  *result = error.into();

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_typedarray(
  env: *mut Env,
  ty: napi_typedarray_type,
  length: usize,
  arraybuffer: napi_value,
  byte_offset: usize,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let ab = transmute::<napi_value, v8::Local<v8::Value>>(arraybuffer);
  let ab = v8::Local::<v8::ArrayBuffer>::try_from(ab).unwrap();
  let typedarray: v8::Local<v8::Value> = match ty {
    napi_uint8_array => {
      v8::Uint8Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_uint8_clamped_array => {
      v8::Uint8ClampedArray::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int8_array => {
      v8::Int8Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_uint16_array => {
      v8::Uint16Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int16_array => {
      v8::Int16Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_uint32_array => {
      v8::Uint32Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int32_array => {
      v8::Int32Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_float32_array => {
      v8::Float32Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_float64_array => {
      v8::Float64Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_bigint64_array => {
      v8::BigInt64Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_biguint64_array => {
      v8::BigUint64Array::new(&mut env.scope(), ab, byte_offset, length)
        .unwrap()
        .into()
    }
    _ => {
      return Err(Error::InvalidArg);
    }
  };
  *result = typedarray.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_uint32(
  env: *mut Env,
  value: u32,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value: v8::Local<v8::Value> =
    v8::Number::new(&mut env.scope(), value as f64).into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_make_callback(
  env: *mut Env,
  async_context: *mut c_void,
  recv: napi_value,
  func: napi_value,
  argc: isize,
  argv: *const napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  check_arg_option!(recv);
  if argc > 0 {
    check_arg!(argv);
  }

  if !async_context.is_null() {
    eprintln!("napi_make_callback: async_context is not supported");
  }

  let recv = transmute::<napi_value, v8::Local<v8::Value>>(recv);
  let func = transmute::<napi_value, v8::Local<v8::Value>>(func);

  let func = v8::Local::<v8::Function>::try_from(func)
    .map_err(|_| Error::FunctionExpected)?;
  let argv: &[v8::Local<v8::Value>] =
    transmute(std::slice::from_raw_parts(argv, argc as usize));
  let ret = func.call(&mut env.scope(), recv, argv);
  *result = transmute::<Option<v8::Local<v8::Value>>, napi_value>(ret);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_bigint_int64(
  env: *mut Env,
  value: napi_value,
  result: *mut i64,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let bigint = value.to_big_int(&mut env.scope()).unwrap();
  *result = bigint.i64_value().0;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_bigint_uint64(
  env: *mut Env,
  value: napi_value,
  result: *mut u64,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let bigint = value.to_big_int(&mut env.scope()).unwrap();
  *result = bigint.u64_value().0;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_bigint_words(
  env: *mut Env,
  value: napi_value,
  sign_bit: *mut i32,
  size: *mut usize,
  out_words: *mut u64,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let bigint = value.to_big_int(&mut env.scope()).unwrap();

  let out_words = std::slice::from_raw_parts_mut(out_words, *size);
  let mut words = Vec::with_capacity(bigint.word_count());
  let (sign, _) = bigint.to_words_array(words.as_mut_slice());
  *sign_bit = sign as i32;

  for (i, word) in out_words.iter_mut().enumerate() {
    *word = words[i];
  }

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_bool(
  env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = value.boolean_value(&mut env.scope());
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_double(
  env: *mut Env,
  value: napi_value,
  result: *mut f64,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = value.number_value(&mut env.scope()).unwrap();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_external(
  _env: *mut Env,
  value: napi_value,
  result: *mut *mut c_void,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let ext = v8::Local::<v8::External>::try_from(value).unwrap();
  *result = ext.value();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_int32(
  env: *mut Env,
  value: napi_value,
  result: *mut i32,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = value.int32_value(&mut env.scope()).unwrap();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_int64(
  env: *mut Env,
  value: napi_value,
  result: *mut i64,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = value.integer_value(&mut env.scope()).unwrap();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_string_latin1(
  env: *mut Env,
  value: napi_value,
  buf: *mut u8,
  bufsize: usize,
  result: *mut usize,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);

  if !value.is_string() && !value.is_string_object() {
    return Err(Error::StringExpected);
  }

  let v8str = value.to_string(&mut env.scope()).unwrap();
  let string_len = v8str.utf8_length(&mut env.scope());

  if buf.is_null() {
    *result = string_len;
  } else if bufsize != 0 {
    let buffer = std::slice::from_raw_parts_mut(buf, bufsize - 1);
    let copied = v8str.write_one_byte(
      &mut env.scope(),
      buffer,
      0,
      v8::WriteOptions::NO_NULL_TERMINATION,
    );
    buf.add(copied).write(0);
    if !result.is_null() {
      *result = copied;
    }
  } else if !result.is_null() {
    *result = string_len;
  }

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_string_utf8(
  env: *mut Env,
  value: napi_value,
  buf: *mut u8,
  bufsize: usize,
  result: *mut usize,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);

  if !value.is_string() && !value.is_string_object() {
    return Err(Error::StringExpected);
  }

  let v8str = value.to_string(&mut env.scope()).unwrap();
  let string_len = v8str.utf8_length(&mut env.scope());

  if buf.is_null() {
    *result = string_len;
  } else if bufsize != 0 {
    let buffer = std::slice::from_raw_parts_mut(buf, bufsize - 1);
    let copied = v8str.write_utf8(
      &mut env.scope(),
      buffer,
      None,
      v8::WriteOptions::NO_NULL_TERMINATION
        | v8::WriteOptions::REPLACE_INVALID_UTF8,
    );
    buf.add(copied).write(0);
    if !result.is_null() {
      *result = copied;
    }
  } else if !result.is_null() {
    *result = string_len;
  }

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_string_utf16(
  env: *mut Env,
  value: napi_value,
  buf: *mut u16,
  bufsize: usize,
  result: *mut usize,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);

  if !value.is_string() && !value.is_string_object() {
    return Err(Error::StringExpected);
  }

  let v8str = value.to_string(&mut env.scope()).unwrap();
  let string_len = v8str.length();

  if buf.is_null() {
    *result = string_len;
  } else if bufsize != 0 {
    let buffer = std::slice::from_raw_parts_mut(buf, bufsize - 1);
    let copied = v8str.write(
      &mut env.scope(),
      buffer,
      0,
      v8::WriteOptions::NO_NULL_TERMINATION,
    );
    buf.add(copied).write(0);
    if !result.is_null() {
      *result = copied;
    }
  } else if !result.is_null() {
    *result = string_len;
  }

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_value_uint32(
  env: *mut Env,
  value: napi_value,
  result: *mut u32,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = value.uint32_value(&mut env.scope()).unwrap();
  Ok(())
}

// TODO
#[napi_sym::napi_sym]
fn napi_add_finalizer(
  _env: *mut Env,
  _js_object: napi_value,
  _native_object: *const c_void,
  _finalize_cb: napi_finalize,
  _finalize_hint: *const c_void,
  _result: *mut napi_ref,
) -> Result {
  eprintln!("napi_add_finalizer is not yet supported.");
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_adjust_external_memory(
  env: *mut Env,
  change_in_bytes: i64,
  adjusted_value: &mut i64,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let isolate = &mut *env.isolate_ptr;
  *adjusted_value =
    isolate.adjust_amount_of_external_allocated_memory(change_in_bytes);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_call_function(
  env: *mut Env,
  recv: napi_value,
  func: napi_value,
  argc: usize,
  argv: *const napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let recv = transmute::<napi_value, v8::Local<v8::Value>>(recv);
  let func = transmute::<napi_value, v8::Local<v8::Value>>(func);
  let func = v8::Local::<v8::Function>::try_from(func)
    .map_err(|_| Error::FunctionExpected)?;

  let argv: &[v8::Local<v8::Value>] =
    transmute(std::slice::from_raw_parts(argv, argc as usize));
  let ret = func.call(&mut env.scope(), recv, argv);
  if !result.is_null() {
    *result = transmute::<Option<v8::Local<v8::Value>>, napi_value>(ret);
  }

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_close_escapable_handle_scope(
  env: *mut Env,
  _scope: napi_escapable_handle_scope,
) -> Result {
  let mut _env = &mut *(env as *mut Env);
  // TODO: do this properly
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_close_handle_scope(env: *mut Env, scope: napi_handle_scope) -> Result {
  let env = &mut *(env as *mut Env);
  if env.open_handle_scopes == 0 {
    return Err(Error::HandleScopeMismatch);
  }
  let _scope = &mut *(scope as *mut v8::HandleScope);
  env.open_handle_scopes -= 1;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_define_class(
  env_ptr: *mut Env,
  name: *const c_char,
  length: isize,
  constructor: napi_callback,
  callback_data: *mut c_void,
  property_count: usize,
  properties: *const napi_property_descriptor,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env_ptr.as_mut().ok_or(Error::InvalidArg)?;
  check_arg!(result);
  // check_arg!(constructor as *const c_void);

  if property_count > 0 {
    check_arg!(properties);
  }

  let name = if length == -1 {
    std::ffi::CStr::from_ptr(name)
      .to_str()
      .map_err(|_| Error::InvalidArg)?
  } else {
    let slice = std::slice::from_raw_parts(name as *const u8, length as usize);
    std::str::from_utf8(slice).unwrap()
  };

  let tpl =
    create_function_template(env_ptr, Some(name), constructor, callback_data);

  let scope = &mut env.scope();
  let napi_properties: &[napi_property_descriptor] =
    std::slice::from_raw_parts(properties, property_count);

  for p in napi_properties {
    let name = if !p.utf8name.is_null() {
      let name_str = CStr::from_ptr(p.utf8name).to_str().unwrap();
      v8::String::new(scope, name_str).unwrap()
    } else {
      transmute::<napi_value, v8::Local<v8::String>>(p.name)
    };

    let method = p.method;
    let getter = p.getter;
    let setter = p.setter;

    if getter.is_some() || setter.is_some() {
      let getter: Option<v8::Local<v8::FunctionTemplate>> = if getter.is_some()
      {
        Some(create_function_template(env_ptr, None, p.getter, p.data))
      } else {
        None
      };
      let setter: Option<v8::Local<v8::FunctionTemplate>> = if setter.is_some()
      {
        Some(create_function_template(env_ptr, None, p.setter, p.data))
      } else {
        None
      };

      let mut accessor_property = v8::NONE;
      if getter.is_some()
        && setter.is_some()
        && (p.attributes & napi_writable) == 0
      {
        accessor_property = accessor_property | v8::READ_ONLY;
      }
      if p.attributes & napi_enumerable == 0 {
        accessor_property = accessor_property | v8::DONT_ENUM;
      }
      if p.attributes & napi_configurable == 0 {
        accessor_property = accessor_property | v8::DONT_DELETE;
      }

      let proto = tpl.prototype_template(scope);
      proto.set_accessor_property(
        name.into(),
        getter,
        setter,
        accessor_property,
      );

      // // TODO: use set_accessor & set_accessor_with_setter
      // match (getter, setter) {
      //   (Some(getter), None) => {
      //     proto.set(name.into(), getter.into());
      //   }
      //   (Some(getter), Some(setter)) => {
      //     proto.set(name.into(), getter.into());
      //     proto.set(name.into(), setter.into());
      //   }
      //   (None, Some(setter)) => {
      //     proto.set(name.into(), setter.into());
      //   }
      //   (None, None) => unreachable!(),
      // }
    } else if method.is_some() {
      let function = create_function_template(env_ptr, None, p.method, p.data);
      let proto = tpl.prototype_template(scope);
      proto.set(name.into(), function.into());
    } else {
      let proto = tpl.prototype_template(scope);
      proto.set(
        name.into(),
        transmute::<napi_value, v8::Local<v8::Data>>(p.value),
      );
    }
  }

  let value: v8::Local<v8::Value> = tpl.get_function(scope).unwrap().into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_define_properties(
  env_ptr: *mut Env,
  obj: napi_value,
  property_count: usize,
  properties: *const napi_property_descriptor,
) -> Result {
  let env: &mut Env = env_ptr.as_mut().ok_or(Error::InvalidArg)?;
  let scope = &mut env.scope();
  let object = transmute::<napi_value, v8::Local<v8::Object>>(obj);
  let properties = std::slice::from_raw_parts(properties, property_count);

  for property in properties {
    let name = if !property.utf8name.is_null() {
      let name_str = CStr::from_ptr(property.utf8name).to_str().unwrap();
      v8::String::new(scope, name_str).unwrap()
    } else {
      transmute::<napi_value, v8::Local<v8::String>>(property.name)
    };

    let method_ptr = property.method;

    if method_ptr.is_some() {
      let function: v8::Local<v8::Value> = {
        let function =
          create_function(env_ptr, None, property.method, property.data);
        function.into()
      };
      object.set(scope, name.into(), function).unwrap();
    }
  }

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_delete_element(
  env: *mut Env,
  value: napi_value,
  index: u32,
  result: *mut bool,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  *result = obj.delete_index(&mut env.scope(), index).unwrap_or(false);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_delete_property(
  env: *mut Env,
  value: napi_value,
  key: napi_value,
  result: *mut bool,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  *result = obj
    .delete(
      &mut env.scope(),
      transmute::<napi_value, v8::Local<v8::Value>>(key),
    )
    .unwrap_or(false);
  Ok(())
}

// TODO: properly implement ref counting stuff
#[napi_sym::napi_sym]
fn napi_delete_reference(env: *mut Env, _nref: napi_ref) -> Result {
  let mut _env = &mut *(env as *mut Env);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_detach_arraybuffer(_env: *mut Env, value: napi_value) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let ab = v8::Local::<v8::ArrayBuffer>::try_from(value).unwrap();
  ab.detach();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_escape_handle<'s>(
  _env: *mut Env,
  _handle_scope: napi_escapable_handle_scope,
  escapee: napi_value<'s>,
  result: *mut napi_value<'s>,
) -> Result {
  // TODO
  *result = escapee;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_all_property_names(_env: *mut Env) -> Result {
  // TODO
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_and_clear_last_exception(
  env: *mut Env,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  // TODO: just return undefined for now we don't cache
  // exceptions in env.
  let value: v8::Local<v8::Value> = v8::undefined(&mut env.scope()).into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_array_length(
  _env: *mut Env,
  value: napi_value,
  result: *mut u32,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = v8::Local::<v8::Array>::try_from(value).unwrap().length();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_arraybuffer_info(
  _env: *mut Env,
  value: napi_value,
  data: *mut *mut u8,
  length: *mut usize,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let buf = v8::Local::<v8::ArrayBuffer>::try_from(value).unwrap();
  if !data.is_null() {
    *data = get_array_buffer_ptr(buf);
  }
  *length = buf.byte_length();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_boolean(
  env: *mut Env,
  value: bool,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value: v8::Local<v8::Value> =
    v8::Boolean::new(env.isolate(), value).into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_buffer_info(
  env: *mut Env,
  value: napi_value,
  data: *mut *mut u8,
  length: *mut usize,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let buf = v8::Local::<v8::Uint8Array>::try_from(value).unwrap();
  let buffer_name = v8::String::new(&mut env.scope(), "buffer").unwrap();
  let abuf = v8::Local::<v8::ArrayBuffer>::try_from(
    buf.get(&mut env.scope(), buffer_name.into()).unwrap(),
  )
  .unwrap();
  if !data.is_null() {
    *data = get_array_buffer_ptr(abuf);
  }
  *length = abuf.byte_length();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_cb_info(
  _env: *mut Env,
  cbinfo: napi_callback_info,
  argc: *mut i32,
  argv: *mut napi_value,
  this_arg: *mut napi_value,
  cb_data: *mut *mut c_void,
) -> Result {
  let cbinfo: &CallbackInfo = &*(cbinfo as *const CallbackInfo);
  let args = &*(cbinfo.args as *const v8::FunctionCallbackArguments);

  if !cb_data.is_null() {
    *cb_data = cbinfo.cb_info;
  }

  if !this_arg.is_null() {
    let mut this = args.this();
    *this_arg = this.into();
  }

  let len = args.length();
  let mut v_argc = len;
  if !argc.is_null() {
    *argc = len;
  }

  if !argv.is_null() {
    let mut v_argv = std::slice::from_raw_parts_mut(argv, v_argc as usize);
    for i in 0..v_argc {
      let mut arg = args.get(i);
      v_argv[i as usize] = arg.into();
    }
  }

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_dataview_info(
  env: *mut Env,
  value: napi_value,
  data: *mut *mut u8,
  length: *mut usize,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let buf = v8::Local::<v8::DataView>::try_from(value).unwrap();
  let buffer_name = v8::String::new(&mut env.scope(), "buffer").unwrap();
  let abuf = v8::Local::<v8::ArrayBuffer>::try_from(
    buf.get(&mut env.scope(), buffer_name.into()).unwrap(),
  )
  .unwrap();
  if !data.is_null() {
    *data = get_array_buffer_ptr(abuf);
  }
  *length = abuf.byte_length();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_date_value(
  env: *mut Env,
  value: napi_value,
  result: *mut f64,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let date = v8::Local::<v8::Date>::try_from(value).unwrap();
  *result = date.number_value(&mut env.scope()).unwrap();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_element(
  env: *mut Env,
  object: napi_value,
  index: u32,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let object = transmute::<napi_value, v8::Local<v8::Value>>(object);
  let array = v8::Local::<v8::Array>::try_from(object).unwrap();
  let value: v8::Local<v8::Value> =
    array.get_index(&mut env.scope(), index).unwrap();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_global(env: *mut Env, result: *mut napi_value) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let context = &mut env.scope().get_current_context();
  let global = context.global(&mut env.scope());
  let value: v8::Local<v8::Value> = global.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_instance_data(env: *mut Env, result: *mut *mut c_void) -> Result {
  let env = &mut *(env as *mut Env);
  let shared = env.shared();
  *result = shared.instance_data;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_last_error_info(
  _env: *mut Env,
  error_code: *mut *const napi_extended_error_info,
) -> Result {
  let err_info = Box::new(napi_extended_error_info {
    error_message: std::ptr::null(),
    engine_reserved: std::ptr::null_mut(),
    engine_error_code: 0,
    status_code: napi_ok,
  });

  *error_code = Box::into_raw(err_info);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_named_property(
  env: *mut Env,
  object: napi_value,
  utf8_name: *const c_char,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let object = transmute::<napi_value, v8::Local<v8::Value>>(object);
  let utf8_name = std::ffi::CStr::from_ptr(utf8_name);
  let name =
    v8::String::new(&mut env.scope(), &utf8_name.to_string_lossy()).unwrap();
  let value: v8::Local<v8::Value> = object
    .to_object(&mut env.scope())
    .unwrap()
    .get(&mut env.scope(), name.into())
    .unwrap();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_new_target(
  _env: &mut Env,
  cbinfo: &CallbackInfo,
  result: &mut v8::Local<v8::Value>,
) -> Result {
  let info = &*(cbinfo.args as *const v8::FunctionCallbackArguments);
  *result = info.new_target();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_null(env: *mut Env, result: *mut napi_value) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let value: v8::Local<v8::Value> = v8::null(env.isolate()).into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_property(
  env: *mut Env,
  object: napi_value,
  key: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let object = transmute::<napi_value, v8::Local<v8::Object>>(object);
  let key = transmute::<napi_value, v8::Local<v8::Value>>(key);
  let value: v8::Local<v8::Value> = object.get(&mut env.scope(), key).unwrap();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_property_names(
  env: *mut Env,
  object: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let object = transmute::<napi_value, v8::Local<v8::Value>>(object);
  let array: v8::Local<v8::Array> = object
    .to_object(&mut env.scope())
    .unwrap()
    .get_property_names(&mut env.scope(), Default::default())
    .unwrap();
  let value: v8::Local<v8::Value> = array.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_prototype(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  let proto = obj.get_prototype(&mut env.scope()).unwrap();
  *result = proto.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_reference_value(
  env: *mut Env,
  reference: napi_ref,
  result: *mut napi_value,
) -> Result {
  // TODO
  let _env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let value = transmute::<napi_ref, v8::Local<v8::Value>>(reference);
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_typedarray_info(
  env: *mut Env,
  value: napi_value,
  data: *mut *mut u8,
  length: *mut usize,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let buf = v8::Local::<v8::TypedArray>::try_from(value).unwrap();
  let buffer_name = v8::String::new(&mut env.scope(), "buffer").unwrap();
  let abuf = v8::Local::<v8::ArrayBuffer>::try_from(
    buf.get(&mut env.scope(), buffer_name.into()).unwrap(),
  )
  .unwrap();
  if !data.is_null() {
    *data = get_array_buffer_ptr(abuf);
  }
  *length = abuf.byte_length();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_get_undefined(env: *mut Env, result: *mut napi_value) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value: v8::Local<v8::Value> = v8::undefined(env.isolate()).into();
  *result = value.into();
  Ok(())
}

pub const NAPI_VERSION: u32 = 8;

#[napi_sym::napi_sym]
fn napi_get_version(_: napi_env, version: *mut u32) -> Result {
  *version = NAPI_VERSION;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_has_element(
  env: *mut Env,
  value: napi_value,
  index: u32,
  result: *mut bool,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  *result = obj.has_index(&mut env.scope(), index).unwrap_or(false);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_has_named_property(
  env: *mut Env,
  value: napi_value,
  key: *const c_char,
  result: *mut bool,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  let key = CStr::from_ptr(key).to_str().unwrap();
  let key = v8::String::new(&mut env.scope(), key).unwrap();
  *result = obj.has(&mut env.scope(), key.into()).unwrap_or(false);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_has_own_property(
  env: *mut Env,
  object: napi_value,
  key: napi_value,
  result: *mut bool,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(object);
  let object = value.to_object(&mut env.scope()).unwrap();

  let key = transmute::<napi_value, v8::Local<v8::Value>>(key);
  if !key.is_name() {
    return Err(Error::NameExpected);
  }

  let maybe = object
    .has_own_property(
      &mut env.scope(),
      v8::Local::<v8::Name>::try_from(key).unwrap(),
    )
    .unwrap_or(false);

  *result = maybe;
  if !maybe {
    return Err(Error::GenericFailure);
  }

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_has_property(
  env: *mut Env,
  value: napi_value,
  key: napi_value,
  result: *mut bool,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  *result = obj
    .has(
      &mut env.scope(),
      transmute::<napi_value, v8::Local<v8::Value>>(key),
    )
    .unwrap_or(false);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_instanceof(
  env: *mut Env,
  value: napi_value,
  constructor: napi_value,
  result: *mut bool,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  check_arg_option!(constructor);
  check_arg_option!(value);

  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let constructor = transmute::<napi_value, v8::Local<v8::Value>>(constructor);
  let ctor = constructor
    .to_object(&mut env.scope())
    .ok_or(Error::ObjectExpected)?;
  if !ctor.is_function() {
    return Err(Error::FunctionExpected);
  }
  let maybe = value.instance_of(&mut env.scope(), ctor);
  match maybe {
    Some(res) => {
      *result = res;
      Ok(())
    }
    None => Err(Error::GenericFailure),
  }
}

#[napi_sym::napi_sym]
fn napi_is_array(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = value.is_array();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_is_arraybuffer(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = value.is_array_buffer();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_is_buffer(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  // TODO: should we assume Buffer as Uint8Array in Deno?
  // or use std/node polyfill?
  *result = value.is_typed_array();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_is_dataview(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = value.is_data_view();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_is_date(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = value.is_date();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_is_detached_arraybuffer(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let _ab = v8::Local::<v8::ArrayBuffer>::try_from(value).unwrap();
  *result = _ab.was_detached();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_is_error(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  // TODO
  *result = value.is_object();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_is_exception_pending(env: *mut Env, result: *mut bool) -> Result {
  let mut _env = &mut *(env as *mut Env);
  // TODO
  *result = false;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_is_promise(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = value.is_promise();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_is_typedarray(
  _env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  *result = value.is_typed_array();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_new_instance(
  env: *mut Env,
  constructor: napi_value,
  argc: usize,
  argv: *const napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let constructor = transmute::<napi_value, v8::Local<v8::Value>>(constructor);
  let constructor = v8::Local::<v8::Function>::try_from(constructor).unwrap();
  let args: &[v8::Local<v8::Value>] =
    transmute(std::slice::from_raw_parts(argv, argc));
  let inst = constructor.new_instance(&mut env.scope(), args).unwrap();
  let value: v8::Local<v8::Value> = inst.into();
  *result = value.into();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_object_freeze(env: &mut Env, object: v8::Local<v8::Value>) -> Result {
  let object = object.to_object(&mut env.scope()).unwrap();
  let maybe =
    object.set_integrity_level(&mut env.scope(), v8::IntegrityLevel::Frozen);

  match maybe {
    Some(_) => Ok(()),
    None => Err(Error::GenericFailure),
  }
}

#[napi_sym::napi_sym]
fn napi_object_seal(env: &mut Env, object: v8::Local<v8::Value>) -> Result {
  let object = object.to_object(&mut env.scope()).unwrap();
  let maybe =
    object.set_integrity_level(&mut env.scope(), v8::IntegrityLevel::Sealed);

  match maybe {
    Some(_) => Ok(()),
    None => Err(Error::GenericFailure),
  }
}

#[napi_sym::napi_sym]
fn napi_open_escapable_handle_scope(
  _env: *mut Env,
  _result: *mut napi_escapable_handle_scope,
) -> Result {
  // TODO: do this properly
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_open_handle_scope(
  env: *mut Env,
  _result: *mut napi_handle_scope,
) -> Result {
  let env = &mut *(env as *mut Env);

  // *result = &mut env.scope() as *mut _ as napi_handle_scope;
  env.open_handle_scopes += 1;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_reference_ref() -> Result {
  // TODO
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_reference_unref() -> Result {
  // TODO
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_reject_deferred(
  env: *mut Env,
  deferred: napi_deferred,
  error: napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let deferred_ptr =
    NonNull::new_unchecked(deferred as *mut v8::PromiseResolver);
  // TODO(@littledivy): Use Global::from_raw instead casting to local.
  // SAFETY: Isolate is still alive unless the module is doing something weird,
  // global data is the size of a pointer.
  // Global pointer is obtained from napi_create_promise
  let resolver = transmute::<
    NonNull<v8::PromiseResolver>,
    v8::Local<v8::PromiseResolver>,
  >(deferred_ptr);
  resolver
    .reject(
      &mut env.scope(),
      transmute::<napi_value, v8::Local<v8::Value>>(error),
    )
    .unwrap();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_remove_wrap(env: *mut Env, value: napi_value) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  let shared = &*(env.shared as *const EnvShared);
  let napi_wrap = v8::Local::new(&mut env.scope(), &shared.napi_wrap);
  obj.delete_private(&mut env.scope(), napi_wrap).unwrap();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_resolve_deferred(
  env: *mut Env,
  deferred: napi_deferred,
  result: napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let deferred_ptr =
    NonNull::new_unchecked(deferred as *mut v8::PromiseResolver);
  // TODO(@littledivy): Use Global::from_raw instead casting to local.
  // SAFETY: Isolate is still alive unless the module is doing something weird,
  // global data is the size of a pointer.
  // Global pointer is obtained from napi_create_promise
  let resolver = transmute::<
    NonNull<v8::PromiseResolver>,
    v8::Local<v8::PromiseResolver>,
  >(deferred_ptr);
  resolver
    .resolve(
      &mut env.scope(),
      transmute::<napi_value, v8::Local<v8::Value>>(result),
    )
    .unwrap();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_run_script(
  env: *mut Env,
  script: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  let script = transmute::<napi_value, v8::Local<v8::Value>>(script);
  if !script.is_string() {
    return Err(Error::StringExpected);
  }
  let script = script.to_string(&mut env.scope()).unwrap();

  let script = v8::Script::compile(&mut env.scope(), script, None);
  if script.is_none() {
    return Err(Error::GenericFailure);
  }
  let script = script.unwrap();
  let rv = script.run(&mut env.scope());

  if let Some(rv) = rv {
    *result = rv.into();
  } else {
    return Err(Error::GenericFailure);
  }

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_set_element(
  env: *mut Env,
  object: napi_value,
  index: u32,
  value: napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let object = transmute::<napi_value, v8::Local<v8::Value>>(object);
  let array = v8::Local::<v8::Array>::try_from(object).unwrap();
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  array.set_index(&mut env.scope(), index, value).unwrap();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_set_instance_data(
  env: *mut Env,
  data: *mut c_void,
  finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
) -> Result {
  let env = &mut *(env as *mut Env);
  let shared = env.shared_mut();
  shared.instance_data = data;
  shared.data_finalize = if !(finalize_cb as *const c_void).is_null() {
    Some(finalize_cb)
  } else {
    None
  };
  shared.data_finalize_hint = finalize_hint;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_set_named_property(
  env: *mut Env,
  object: napi_value,
  name: *const c_char,
  value: napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let name = CStr::from_ptr(name).to_str().unwrap();
  let object = transmute::<napi_value, v8::Local<v8::Object>>(object);
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let name = v8::String::new(&mut env.scope(), name).unwrap();
  object.set(&mut env.scope(), name.into(), value).unwrap();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_set_property(
  env: *mut Env,
  object: napi_value,
  property: napi_value,
  value: napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let object = transmute::<napi_value, v8::Local<v8::Value>>(object);
  let object = object.to_object(&mut env.scope()).unwrap();
  let property = transmute::<napi_value, v8::Local<v8::Value>>(property);
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  object.set(&mut env.scope(), property, value).unwrap();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_strict_equals(
  _env: *mut Env,
  lhs: napi_value,
  rhs: napi_value,
  result: *mut bool,
) -> Result {
  let lhs = transmute::<napi_value, v8::Local<v8::Value>>(lhs);
  let rhs = transmute::<napi_value, v8::Local<v8::Value>>(rhs);
  *result = lhs.strict_equals(rhs);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_throw(env: *mut Env, error: napi_value) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let error = transmute::<napi_value, v8::Local<v8::Value>>(error);
  env.scope().throw_exception(error);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_throw_error(
  env: *mut Env,
  _code: *const c_char,
  msg: *const c_char,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  // let code = CStr::from_ptr(code).to_str().unwrap();
  let msg = CStr::from_ptr(msg).to_str().unwrap();

  // let code = v8::String::new(&mut env.scope(), code).unwrap();
  let msg = v8::String::new(&mut env.scope(), msg).unwrap();

  let error = v8::Exception::error(&mut env.scope(), msg);
  env.scope().throw_exception(error);

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_throw_range_error(
  env: *mut Env,
  _code: *const c_char,
  msg: *const c_char,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  // let code = CStr::from_ptr(code).to_str().unwrap();
  let msg = CStr::from_ptr(msg).to_str().unwrap();

  // let code = v8::String::new(&mut env.scope(), code).unwrap();
  let msg = v8::String::new(&mut env.scope(), msg).unwrap();

  let error = v8::Exception::range_error(&mut env.scope(), msg);
  env.scope().throw_exception(error);

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_throw_type_error(
  env: *mut Env,
  _code: *const c_char,
  msg: *const c_char,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  // let code = CStr::from_ptr(code).to_str().unwrap();
  let msg = CStr::from_ptr(msg).to_str().unwrap();

  // let code = v8::String::new(&mut env.scope(), code).unwrap();
  let msg = v8::String::new(&mut env.scope(), msg).unwrap();

  let error = v8::Exception::type_error(&mut env.scope(), msg);
  env.scope().throw_exception(error);

  Ok(())
}

pub fn get_value_type(value: v8::Local<v8::Value>) -> Option<napi_valuetype> {
  if value.is_undefined() {
    Some(napi_undefined)
  } else if value.is_null() {
    Some(napi_null)
  } else if value.is_external() {
    Some(napi_external)
  } else if value.is_boolean() {
    Some(napi_boolean)
  } else if value.is_number() {
    Some(napi_number)
  } else if value.is_big_int() {
    Some(napi_bigint)
  } else if value.is_string() {
    Some(napi_string)
  } else if value.is_symbol() {
    Some(napi_symbol)
  } else if value.is_function() {
    Some(napi_function)
  } else if value.is_object() {
    Some(napi_object)
  } else {
    None
  }
}

#[napi_sym::napi_sym]
fn napi_typeof(
  _env: *mut Env,
  value: napi_value,
  result: *mut napi_valuetype,
) -> Result {
  if value.is_none() {
    *result = napi_undefined;
    return Ok(());
  }
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let ty = get_value_type(value);
  if let Some(ty) = ty {
    *result = ty;
    Ok(())
  } else {
    Err(Error::InvalidArg)
  }
}

#[napi_sym::napi_sym]
fn napi_unwrap(
  env: *mut Env,
  value: napi_value,
  result: *mut *mut c_void,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  let shared = &*(env.shared as *const EnvShared);
  let napi_wrap = v8::Local::new(&mut env.scope(), &shared.napi_wrap);
  let ext = obj.get_private(&mut env.scope(), napi_wrap).unwrap();
  let ext = v8::Local::<v8::External>::try_from(ext).unwrap();
  *result = ext.value();
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_wrap(
  env: *mut Env,
  value: napi_value,
  native_object: *mut c_void,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  let value = transmute::<napi_value, v8::Local<v8::Value>>(value);
  let obj = value.to_object(&mut env.scope()).unwrap();
  let shared = &*(env.shared as *const EnvShared);
  let napi_wrap = v8::Local::new(&mut env.scope(), &shared.napi_wrap);
  let ext = v8::External::new(&mut env.scope(), native_object);
  obj.set_private(&mut env.scope(), napi_wrap, ext.into());
  Ok(())
}

#[napi_sym::napi_sym]
fn node_api_throw_syntax_error(
  env: *mut Env,
  _code: *const c_char,
  msg: *const c_char,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  // let code = CStr::from_ptr(code).to_str().unwrap();
  let msg = CStr::from_ptr(msg).to_str().unwrap();

  // let code = v8::String::new(&mut env.scope(), code).unwrap();
  let msg = v8::String::new(&mut env.scope(), msg).unwrap();

  let error = v8::Exception::syntax_error(&mut env.scope(), msg);
  env.scope().throw_exception(error);

  Ok(())
}

#[napi_sym::napi_sym]
fn node_api_create_syntax_error(
  env: *mut Env,
  _code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> Result {
  let env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;

  // let code = transmute::<napi_value, v8::Local<v8::Value>>(code);
  let msg = transmute::<napi_value, v8::Local<v8::Value>>(msg);

  let msg = msg.to_string(&mut env.scope()).unwrap();

  let error = v8::Exception::syntax_error(&mut env.scope(), msg);
  *result = error.into();

  Ok(())
}
