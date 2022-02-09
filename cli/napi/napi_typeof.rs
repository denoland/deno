use deno_core::napi::*;

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
