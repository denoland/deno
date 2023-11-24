// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::v8;
use deno_core::OpState;
use deno_core::ResourceId;
use std::cell::RefCell;
use std::rc::Rc;

pub fn handle_wasm_streaming(
  state: Rc<RefCell<OpState>>,
  scope: &mut v8::HandleScope,
  value: v8::Local<v8::Value>,
  mut wasm_streaming: v8::WasmStreaming,
) {
  let (url, rid) = match compile_response(scope, value) {
    Ok(Some((url, rid))) => (url, rid),
    Ok(None) => {
      // 2.7
      wasm_streaming.finish();
      return;
    }
    Err(e) => {
      // 2.8
      wasm_streaming.abort(None);
      return;
    }
  };

  wasm_streaming.set_url(&url);

  tokio::task::spawn_local(async move {
    loop {
      let resource = state.borrow().resource_table.get_any(rid);
      let resource = match resource {
        Ok(r) => r,
        Err(_) => {
          wasm_streaming.abort(None);
          return;
        }
      };

      let bytes = match resource.read(65536).await {
        Ok(bytes) => bytes,
        Err(e) => {
          wasm_streaming.abort(None);
          return;
        }
      };
      if bytes.is_empty() {
        break;
      }

      wasm_streaming.on_bytes_received(&bytes);
    }

    wasm_streaming.finish();
  });
}

// Partially implements https://webassembly.github.io/spec/web-api/#compile-a-potential-webassembly-response
pub fn compile_response(
  scope: &mut v8::HandleScope,
  value: v8::Local<v8::Value>,
) -> Result<Option<(String, ResourceId)>, AnyError> {
  let object = value
    .to_object(scope)
    .ok_or_else(|| type_error("Response is not an object."))?;
  let url = get_string(scope, object, "url")?;

  // 2.3.
  // The spec is ambiguous here, see
  // https://github.com/WebAssembly/spec/issues/1138. The WPT tests expect
  // the raw value of the Content-Type attribute lowercased. We ignore this
  // for file:// because file fetches don't have a Content-Type.
  if !url.starts_with("file://") {
    let headers = get_value(scope, object, "headers")?;
    let content_type = call_method(scope, headers, "get", "Content-Type")?;

    if content_type.to_lowercase() != "application/wasm" {
      return Err(type_error("Response is not a wasm file."));
    }
  }

  // 2.5
  let ok = get_value(scope, object, "ok")?;
  if !ok.is_true() {
    return Err(type_error("Response is not ok."));
  }

  let body = get_value(scope, object, "body")?;

  if body.is_null() {
    return Ok(None);
  }
  let body = body
    .to_object(scope)
    .ok_or_else(|| type_error("Failed to get body object."))?;
  let rid = get_value(scope, body, "rid")?
    .to_uint32(scope)
    .ok_or_else(|| type_error("Failed to get rid."))?
    .value() as ResourceId;

  Ok(Some((url, rid)))
}

fn get_value<'a, 'b>(
  scope: &'b mut v8::HandleScope<'a>,
  obj: v8::Local<'a, v8::Object>,
  key: &'static str,
) -> Result<v8::Local<'a, v8::Value>, AnyError> {
  let key = v8::String::new(scope, key)
    .ok_or_else(|| type_error("Failed to create key."))?;
  Ok(
    obj
      .get(scope, key.into())
      .ok_or_else(|| type_error("Failed to get value."))?,
  )
}

fn get_string(
  scope: &mut v8::HandleScope,
  obj: v8::Local<v8::Object>,
  key: &'static str,
) -> Result<String, AnyError> {
  let key = v8::String::new(scope, key)
    .ok_or_else(|| type_error("Failed to create key."))?;
  let value = obj
    .get(scope, key.into())
    .ok_or_else(|| type_error("Failed to get value."))?;

  Ok(value.to_rust_string_lossy(scope))
}

fn call_method<'a>(
  scope: &mut v8::HandleScope<'a>,
  obj: v8::Local<'a, v8::Value>,
  method: &'static str,
  arg: &'static str,
) -> Result<String, AnyError> {
  let key = v8::String::new(scope, method)
    .ok_or_else(|| type_error("Failed to create key."))?;
  let function = obj
    .to_object(scope)
    .ok_or_else(|| type_error("Failed to create object."))?;
  let function = function
    .get(scope, key.into())
    .ok_or_else(|| type_error("Failed to get value."))?;
  let function: v8::Local<v8::Function> = function.try_into()?;
  let arg = v8::String::new(scope, arg)
    .ok_or_else(|| type_error("Failed to create arg."))?;
  let this = v8::undefined(scope).into();
  Ok(
    function
      .call(scope, this, &[arg.into()])
      .ok_or_else(|| type_error("Failed to call."))?
      .to_rust_string_lossy(scope),
  )
}
