use deno_core::napi::*;
use v8::BackingStore;
use v8::UniqueRef;

use super::function::create_function;
use super::util::get_array_buffer_ptr;

#[napi_sym::napi_sym]
fn napi_create_array(env: napi_env, result: *mut napi_value) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = v8::Array::new(env.scope, 0).into();
  *result = std::mem::transmute::<v8::Local<v8::Value>, napi_value>(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_array_with_length(
  env: napi_env,
  len: i32,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = v8::Array::new(env.scope, len).into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_arraybuffer(
  env: napi_env,
  len: usize,
  data: *mut *mut u8,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value = v8::ArrayBuffer::new(env.scope, len);
  if !data.is_null() {
    *data = get_array_buffer_ptr(value);
  }
  let value: v8::Local<v8::Value> = value.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_bigint_int64(
  env: napi_env,
  value: i64,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> =
    v8::BigInt::new_from_i64(env.scope, value).into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_bigint_uint64(
  env: napi_env,
  value: u64,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> =
    v8::BigInt::new_from_u64(env.scope, value).into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_bigint_words(
  env: napi_env,
  sign_bit: bool,
  words: *const u64,
  word_count: usize,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = v8::BigInt::new_from_words(
    env.scope,
    sign_bit,
    std::slice::from_raw_parts(words, word_count),
  )
  .unwrap()
  .into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_buffer(
  env: napi_env,
  len: usize,
  data: *mut *mut u8,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value = v8::ArrayBuffer::new(env.scope, len);
  if !data.is_null() {
    *data = get_array_buffer_ptr(value);
  }
  let value = v8::Uint8Array::new(env.scope, value, 0, len).unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_buffer_copy(
  env: napi_env,
  len: usize,
  data: *mut u8,
  result_data: *mut *mut u8,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value = v8::ArrayBuffer::new(env.scope, len);
  let ptr = get_array_buffer_ptr(value);
  std::ptr::copy(data, ptr, len);
  if !result_data.is_null() {
    *result_data = ptr;
  }
  let value = v8::Uint8Array::new(env.scope, value, 0, len).unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_coerce_to_bool(
  env: napi_env,
  value: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let coerced = value.to_boolean(env.scope);
  let value: v8::Local<v8::Value> = coerced.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_coerce_to_number(
  env: napi_env,
  value: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let coerced = value.to_number(env.scope).unwrap();
  let value: v8::Local<v8::Value> = coerced.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_coerce_to_object(
  env: napi_env,
  value: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let coerced = value.to_object(env.scope).unwrap();
  let value: v8::Local<v8::Value> = coerced.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_coerce_to_string(
  env: napi_env,
  value: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let coerced = value.to_string(env.scope).unwrap();
  let value: v8::Local<v8::Value> = coerced.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_dataview(
  env: napi_env,
  len: usize,
  data: *mut *mut u8,
  byte_offset: usize,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value = v8::ArrayBuffer::new(env.scope, len);
  if !data.is_null() {
    *data = get_array_buffer_ptr(value);
  }
  let context = env.scope.get_current_context();
  let global = context.global(env.scope);
  let data_view_name = v8::String::new(env.scope, "DataView").unwrap();
  let data_view = global.get(env.scope, data_view_name.into()).unwrap();
  let data_view = v8::Local::<v8::Function>::try_from(data_view).unwrap();
  let byte_offset = v8::Number::new(env.scope, byte_offset as f64);
  let byte_length = v8::Number::new(env.scope, len as f64);
  let value = data_view
    .new_instance(
      env.scope,
      &[value.into(), byte_offset.into(), byte_length.into()],
    )
    .unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_date(
  env: napi_env,
  time: f64,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> =
    v8::Date::new(env.scope, time).unwrap().into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_double(
  env: napi_env,
  value: f64,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = v8::Number::new(env.scope, value).into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_error(
  env: napi_env,
  code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  let _code: v8::Local<v8::Value> = std::mem::transmute(code);
  let msg: v8::Local<v8::Value> = std::mem::transmute(msg);

  let msg = msg.to_string(env.scope).unwrap();

  let error = v8::Exception::error(env.scope, msg);
  *result = std::mem::transmute(error);

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_external(
  env: napi_env,
  value: *mut c_void,
  _finalize_cb: napi_finalize,
  _finalize_hint: *mut c_void,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = v8::External::new(env.scope, value).into();
  // TODO: finalization
  *result = std::mem::transmute(value);
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
  env: napi_env,
  data: *mut c_void,
  byte_length: usize,
  _finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let _slice = std::slice::from_raw_parts(data as *mut u8, byte_length);
  // TODO: finalization
  let store: UniqueRef<BackingStore> =
    std::mem::transmute(v8__ArrayBuffer__NewBackingStore__with_data(
      data,
      byte_length,
      backing_store_deleter_callback,
      finalize_hint,
    ));

  let ab = v8::ArrayBuffer::with_backing_store(env.scope, &store.make_shared());
  let value: v8::Local<v8::Value> = ab.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_external_buffer(
  env: napi_env,
  byte_length: isize,
  data: *mut c_void,
  _finalize_cb: napi_finalize,
  _finalize_hint: *mut c_void,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
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
  let ab = v8::ArrayBuffer::with_backing_store(env.scope, &store.make_shared());
  let value = v8::Uint8Array::new(env.scope, ab, 0, slice.len()).unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_function(
  env: &mut Env,
  name: *const u8,
  length: isize,
  cb: napi_callback,
  cb_info: napi_callback_info,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let name = match name.is_null() {
    true => None,
    false => Some(name),
  };
  let name = name.map(|name| {
    if length == -1 {
      std::ffi::CStr::from_ptr(name as *const _).to_str().unwrap()
    } else {
      let name = std::slice::from_raw_parts(name, length as usize);
      std::str::from_utf8(name).unwrap()
    }
  });
  let function = create_function(env, name, cb, cb_info);
  let value: v8::Local<v8::Value> = function.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_int32(
  env: napi_env,
  value: i32,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> =
    v8::Number::new(env.scope, value as f64).into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_int64(
  env: napi_env,
  value: i64,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> =
    v8::Number::new(env.scope, value as f64).into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_object(env: napi_env, result: *mut napi_value) -> Result {
  let mut env = &mut *(env as *mut Env);
  let object = v8::Object::new(env.scope);
  *result = std::mem::transmute(object);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_promise(
  env: napi_env,
  deferred: *mut napi_deferred,
  promise_out: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let resolver = v8::PromiseResolver::new(env.scope).unwrap();
  let mut global = v8::Global::new(env.scope, resolver);
  let mut global_ptr = global.into_raw();
  let promise: v8::Local<v8::Value> = resolver.get_promise(env.scope).into();
  *deferred = global_ptr.as_mut() as *mut _ as napi_deferred;
  *promise_out = std::mem::transmute(promise);

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_range_error(
  env: napi_env,
  _code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  // let code: v8::Local<v8::Value> = std::mem::transmute(code);
  let msg: v8::Local<v8::Value> = std::mem::transmute(msg);

  let msg = msg.to_string(env.scope).unwrap();

  let error = v8::Exception::range_error(env.scope, msg);
  *result = std::mem::transmute(error);

  Ok(())
}

//

// TODO: properly implement ref counting stuff
#[napi_sym::napi_sym]
fn napi_create_reference(
  env: napi_env,
  value: napi_value,
  _initial_refcount: u32,
  result: *mut napi_ref,
) -> Result {
  let mut _env = &mut *(env as *mut Env);
  *result = value;
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_string_latin1(
  env: napi_env,
  string: *const u8,
  length: isize,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  let string = if length == -1 {
    std::ffi::CStr::from_ptr(string as *const _)
      .to_str()
      .unwrap()
      .as_bytes()
  } else {
    std::slice::from_raw_parts(string, length as usize)
  };
  match v8::String::new_from_one_byte(
    env.scope,
    string,
    v8::NewStringType::Normal,
  ) {
    Some(v8str) => {
      let value: v8::Local<v8::Value> = v8str.into();
      *result = std::mem::transmute(value);
    }
    None => return Err(Error::GenericFailure),
  }

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_string_utf16(
  env: napi_env,
  string: *const u16,
  length: usize,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let string = std::slice::from_raw_parts(string, length);
  let v8str =
    v8::String::new_from_two_byte(env.scope, string, v8::NewStringType::Normal)
      .unwrap();
  let value: v8::Local<v8::Value> = v8str.into();
  *result = std::mem::transmute(value);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_string_utf8(
  env: napi_env,
  string: *const u8,
  length: isize,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  let string = if length == -1 {
    std::ffi::CStr::from_ptr(string as *const _)
      .to_str()
      .unwrap()
  } else {
    let string = std::slice::from_raw_parts(string, length as usize);
    std::str::from_utf8(string).unwrap()
  };
  let v8str = v8::String::new(env.scope, string).unwrap();
  let value: v8::Local<v8::Value> = v8str.into();
  *result = std::mem::transmute(value);

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_symbol(
  env: napi_env,
  description: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let description = match description.is_null() {
    true => None,
    false => Some(
      std::mem::transmute::<napi_value, v8::Local<v8::Value>>(description)
        .to_string(env.scope)
        .unwrap(),
    ),
  };
  let sym = v8::Symbol::new(env.scope, description);
  let local: v8::Local<v8::Value> = sym.into();
  *result = transmute(local);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_type_error(
  env: napi_env,
  _code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  // let code: v8::Local<v8::Value> = std::mem::transmute(code);
  let msg: v8::Local<v8::Value> = std::mem::transmute(msg);

  let msg = msg.to_string(env.scope).unwrap();

  let error = v8::Exception::type_error(env.scope, msg);
  *result = std::mem::transmute(error);

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_typedarray(
  env: napi_env,
  ty: napi_typedarray_type,
  length: usize,
  arraybuffer: napi_value,
  byte_offset: usize,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let ab: v8::Local<v8::Value> = std::mem::transmute(arraybuffer);
  let ab = v8::Local::<v8::ArrayBuffer>::try_from(ab).unwrap();
  let typedarray: v8::Local<v8::Value> = match ty {
    napi_uint8_array => v8::Uint8Array::new(env.scope, ab, byte_offset, length)
      .unwrap()
      .into(),
    napi_uint8_clamped_array => {
      v8::Uint8ClampedArray::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int8_array => v8::Int8Array::new(env.scope, ab, byte_offset, length)
      .unwrap()
      .into(),
    napi_uint16_array => {
      v8::Uint16Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int16_array => v8::Int16Array::new(env.scope, ab, byte_offset, length)
      .unwrap()
      .into(),
    napi_uint32_array => {
      v8::Uint32Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int32_array => v8::Int32Array::new(env.scope, ab, byte_offset, length)
      .unwrap()
      .into(),
    napi_float32_array => {
      v8::Float32Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_float64_array => {
      v8::Float64Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_bigint64_array => {
      v8::BigInt64Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_biguint64_array => {
      v8::BigUint64Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    _ => {
      return Err(Error::InvalidArg);
    }
  };
  *result = std::mem::transmute(typedarray);
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_create_uint32(
  env: napi_env,
  value: u32,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> =
    v8::Number::new(env.scope, value as f64).into();
  *result = std::mem::transmute(value);
  Ok(())
}
