use deno_core::napi::*;

pub fn get_value_type(value: v8::Local<v8::Value>) -> Option<napi_valuetype> {
  if value.is_undefined() {
    return Some(napi_undefined);
  } else if value.is_null() {
    return Some(napi_null);
  } else if value.is_external() {
    return Some(napi_external);
  } else if value.is_boolean() {
    return Some(napi_boolean);
  } else if value.is_number() {
    return Some(napi_number);
  } else if value.is_big_int() {
    return Some(napi_bigint);
  } else if value.is_string() {
    return Some(napi_string);
  } else if value.is_symbol() {
    return Some(napi_symbol);
  } else if value.is_function() {
    return Some(napi_function);
  } else if value.is_object() {
    return Some(napi_object);
  } else {
    return None;
  }
}

#[napi_sym::napi_sym]
fn napi_typeof(
  _env: napi_env,
  value: napi_value,
  result: *mut napi_valuetype,
) -> Result {
  if value.is_null() {
    *result = napi_undefined;
    return Ok(());
  }
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let ty = get_value_type(value);
  if let Some(ty) = ty {
    *result = ty;
    Ok(())
  } else {
    return Err(Error::InvalidArg);
  }
}
