// Copyright 2018-2025 the Deno authors. MIT license.

pub use impl_::*;

pub struct ChildPipeFd(pub i64);

mod impl_ {
  use std::cell::RefCell;
  use std::future::Future;
  use std::io;
  use std::rc::Rc;

  use deno_core::op2;
  use deno_core::serde;
  use deno_core::serde::Serializer;
  use deno_core::serde_json;
  use deno_core::v8;
  use deno_core::CancelFuture;
  use deno_core::OpState;
  use deno_core::RcRef;
  use deno_core::ResourceId;
  use deno_core::ToV8;
  use deno_error::JsErrorBox;
  use deno_process::ipc::IpcJsonStreamError;
  pub use deno_process::ipc::IpcJsonStreamResource;
  pub use deno_process::ipc::IpcRefTracker;
  pub use deno_process::ipc::INITIAL_CAPACITY;
  use serde::Serialize;

  /// Wrapper around v8 value that implements Serialize.
  struct SerializeWrapper<'a, 'b>(
    RefCell<&'b mut v8::HandleScope<'a>>,
    v8::Local<'a, v8::Value>,
  );

  impl<'a, 'b> Serialize for SerializeWrapper<'a, 'b> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
      S: Serializer,
    {
      serialize_v8_value(*self.0.borrow_mut(), self.1, serializer)
    }
  }

  /// Serialize a v8 value directly into a serde serializer.
  /// This allows us to go from v8 values to JSON without having to
  /// deserialize into a `serde_json::Value` and then reserialize to JSON
  fn serialize_v8_value<'a, S: Serializer>(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
    ser: S,
  ) -> Result<S::Ok, S::Error> {
    use serde::ser::Error;
    if value.is_null_or_undefined() {
      ser.serialize_unit()
    } else if value.is_number() || value.is_number_object() {
      let num_value = value.number_value(scope).unwrap();
      if (num_value as i64 as f64) == num_value {
        ser.serialize_i64(num_value as i64)
      } else {
        ser.serialize_f64(num_value)
      }
    } else if value.is_string() {
      let str = deno_core::serde_v8::to_utf8(value.try_into().unwrap(), scope);
      ser.serialize_str(&str)
    } else if value.is_string_object() {
      let str = deno_core::serde_v8::to_utf8(
        value.to_string(scope).ok_or_else(|| {
          S::Error::custom(deno_error::JsErrorBox::generic(
            "toString on string object failed",
          ))
        })?,
        scope,
      );
      ser.serialize_str(&str)
    } else if value.is_boolean() {
      ser.serialize_bool(value.is_true())
    } else if value.is_boolean_object() {
      ser.serialize_bool(value.boolean_value(scope))
    } else if value.is_array() {
      use serde::ser::SerializeSeq;
      let array = value.cast::<v8::Array>();
      let length = array.length();
      let mut seq = ser.serialize_seq(Some(length as usize))?;
      for i in 0..length {
        let element = array.get_index(scope, i).unwrap();
        seq
          .serialize_element(&SerializeWrapper(RefCell::new(scope), element))?;
      }
      seq.end()
    } else if value.is_object() {
      use serde::ser::SerializeMap;
      if value.is_array_buffer_view() {
        let buffer = value.cast::<v8::ArrayBufferView>();
        let mut buf = vec![0u8; buffer.byte_length()];
        let copied = buffer.copy_contents(&mut buf);
        debug_assert_eq!(copied, buf.len());
        return ser.serialize_bytes(&buf);
      }
      let object = value.cast::<v8::Object>();
      // node uses `JSON.stringify`, so to match its behavior (and allow serializing custom objects)
      // we need to respect the `toJSON` method if it exists.
      let to_json_key = v8::String::new_from_utf8(
        scope,
        b"toJSON",
        v8::NewStringType::Internalized,
      )
      .unwrap()
      .into();
      if let Some(to_json) = object.get(scope, to_json_key) {
        if let Ok(to_json) = to_json.try_cast::<v8::Function>() {
          let json_value = to_json.call(scope, object.into(), &[]).unwrap();
          return serialize_v8_value(scope, json_value, ser);
        }
      }

      let keys = object
        .get_own_property_names(
          scope,
          v8::GetPropertyNamesArgs {
            ..Default::default()
          },
        )
        .unwrap();
      let num_keys = keys.length();
      let mut map = ser.serialize_map(Some(num_keys as usize))?;
      for i in 0..num_keys {
        let key = keys.get_index(scope, i).unwrap();
        let key_str = key.to_rust_string_lossy(scope);
        let value = object.get(scope, key).unwrap();
        if value.is_undefined() {
          continue;
        }
        map.serialize_entry(
          &key_str,
          &SerializeWrapper(RefCell::new(scope), value),
        )?;
      }
      map.end()
    } else {
      // TODO(nathanwhit): better error message
      Err(S::Error::custom(JsErrorBox::type_error(format!(
        "Unsupported type: {}",
        value.type_repr()
      ))))
    }
  }

  // Open IPC pipe from bootstrap options.
  #[op2]
  #[smi]
  pub fn op_node_child_ipc_pipe(
    state: &mut OpState,
  ) -> Result<Option<ResourceId>, io::Error> {
    let fd = match state.try_borrow_mut::<crate::ChildPipeFd>() {
      Some(child_pipe_fd) => child_pipe_fd.0,
      None => return Ok(None),
    };
    let ref_tracker = IpcRefTracker::new(state.external_ops_tracker.clone());
    Ok(Some(
      state
        .resource_table
        .add(IpcJsonStreamResource::new(fd, ref_tracker)?),
    ))
  }

  #[derive(Debug, thiserror::Error, deno_error::JsError)]
  pub enum IpcError {
    #[class(inherit)]
    #[error(transparent)]
    Resource(#[from] deno_core::error::ResourceError),
    #[class(inherit)]
    #[error(transparent)]
    IpcJsonStream(#[from] IpcJsonStreamError),
    #[class(inherit)]
    #[error(transparent)]
    Canceled(#[from] deno_core::Canceled),
    #[class(inherit)]
    #[error("failed to serialize json value: {0}")]
    SerdeJson(serde_json::Error),
  }

  #[op2(async)]
  pub fn op_node_ipc_write<'a>(
    scope: &mut v8::HandleScope<'a>,
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
    value: v8::Local<'a, v8::Value>,
    // using an array as an "out parameter".
    // index 0 is a boolean indicating whether the queue is under the limit.
    //
    // ideally we would just return `Result<(impl Future, bool), ..>`, but that's not
    // supported by `op2` currently.
    queue_ok: v8::Local<'a, v8::Array>,
  ) -> Result<impl Future<Output = Result<(), io::Error>>, IpcError> {
    let mut serialized = Vec::with_capacity(64);
    let mut ser = serde_json::Serializer::new(&mut serialized);
    serialize_v8_value(scope, value, &mut ser).map_err(IpcError::SerdeJson)?;
    serialized.push(b'\n');

    let stream = state
      .borrow()
      .resource_table
      .get::<IpcJsonStreamResource>(rid)?;
    let old = stream
      .queued_bytes
      .fetch_add(serialized.len(), std::sync::atomic::Ordering::Relaxed);
    if old + serialized.len() > 2 * INITIAL_CAPACITY {
      // sending messages too fast
      let v = false.to_v8(scope).unwrap(); // Infallible
      queue_ok.set_index(scope, 0, v);
    }
    Ok(async move {
      let cancel = stream.cancel.clone();
      let result = stream
        .clone()
        .write_msg_bytes(&serialized)
        .or_cancel(cancel)
        .await;
      // adjust count even on error
      stream
        .queued_bytes
        .fetch_sub(serialized.len(), std::sync::atomic::Ordering::Relaxed);
      result??;
      Ok(())
    })
  }

  /// Value signaling that the other end ipc channel has closed.
  ///
  /// Node reserves objects of this form (`{ "cmd": "NODE_<something>"`)
  /// for internal use, so we use it here as well to avoid breaking anyone.
  fn stop_sentinel() -> serde_json::Value {
    serde_json::json!({
      "cmd": "NODE_CLOSE"
    })
  }

  #[op2(async)]
  #[serde]
  pub async fn op_node_ipc_read(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
  ) -> Result<serde_json::Value, IpcError> {
    let stream = state
      .borrow()
      .resource_table
      .get::<IpcJsonStreamResource>(rid)?;

    let cancel = stream.cancel.clone();
    let mut stream = RcRef::map(stream, |r| &r.read_half).borrow_mut().await;
    let msgs = stream.read_msg().or_cancel(cancel).await??;
    if let Some(msg) = msgs {
      Ok(msg)
    } else {
      Ok(stop_sentinel())
    }
  }

  #[op2(fast)]
  pub fn op_node_ipc_ref(state: &mut OpState, #[smi] rid: ResourceId) {
    let stream = state
      .resource_table
      .get::<IpcJsonStreamResource>(rid)
      .expect("Invalid resource ID");
    stream.ref_tracker.ref_();
  }

  #[op2(fast)]
  pub fn op_node_ipc_unref(state: &mut OpState, #[smi] rid: ResourceId) {
    let stream = state
      .resource_table
      .get::<IpcJsonStreamResource>(rid)
      .expect("Invalid resource ID");
    stream.ref_tracker.unref();
  }

  #[cfg(test)]
  mod tests {
    use deno_core::v8;
    use deno_core::JsRuntime;
    use deno_core::RuntimeOptions;

    fn wrap_expr(s: &str) -> String {
      format!("(function () {{ return {s}; }})()")
    }

    fn serialize_js_to_json(runtime: &mut JsRuntime, js: String) -> String {
      let val = runtime.execute_script("", js).unwrap();
      let scope = &mut runtime.handle_scope();
      let val = v8::Local::new(scope, val);
      let mut buf = Vec::new();
      let mut ser = deno_core::serde_json::Serializer::new(&mut buf);
      super::serialize_v8_value(scope, val, &mut ser).unwrap();
      String::from_utf8(buf).unwrap()
    }

    #[test]
    fn ipc_serialization() {
      let mut runtime = JsRuntime::new(RuntimeOptions::default());

      let cases = [
        ("'hello'", "\"hello\""),
        ("1", "1"),
        ("1.5", "1.5"),
        ("Number.NaN", "null"),
        ("Infinity", "null"),
        ("Number.MAX_SAFE_INTEGER", &(2i64.pow(53) - 1).to_string()),
        (
          "Number.MIN_SAFE_INTEGER",
          &(-(2i64.pow(53) - 1)).to_string(),
        ),
        ("[1, 2, 3]", "[1,2,3]"),
        ("new Uint8Array([1,2,3])", "[1,2,3]"),
        (
          "{ a: 1.5, b: { c: new ArrayBuffer(5) }}",
          r#"{"a":1.5,"b":{"c":{}}}"#,
        ),
        ("new Number(1)", "1"),
        ("new Boolean(true)", "true"),
        ("true", "true"),
        (r#"new String("foo")"#, "\"foo\""),
        ("null", "null"),
        (
          r#"{ a: "field", toJSON() { return "custom"; } }"#,
          "\"custom\"",
        ),
        (r#"{ a: undefined, b: 1 }"#, "{\"b\":1}"),
      ];

      for (input, expect) in cases {
        let js = wrap_expr(input);
        let actual = serialize_js_to_json(&mut runtime, js);
        assert_eq!(actual, expect);
      }
    }
  }
}
