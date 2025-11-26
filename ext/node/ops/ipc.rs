// Copyright 2018-2025 the Deno authors. MIT license.

pub use impl_::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChildIpcSerialization {
  Json,
  Advanced,
}

impl std::str::FromStr for ChildIpcSerialization {
  type Err = deno_core::anyhow::Error;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "json" => Ok(ChildIpcSerialization::Json),
      "advanced" => Ok(ChildIpcSerialization::Advanced),
      _ => Err(deno_core::anyhow::anyhow!(
        "Invalid serialization type: {}",
        s
      )),
    }
  }
}

pub struct ChildPipeFd(pub i64, pub ChildIpcSerialization);

mod impl_ {
  use std::cell::RefCell;
  use std::future::Future;
  use std::io;
  use std::rc::Rc;

  use deno_core::CancelFuture;
  use deno_core::OpState;
  use deno_core::RcRef;
  use deno_core::ResourceId;
  use deno_core::ToV8;
  use deno_core::op2;
  use deno_core::serde;
  use deno_core::serde::Serializer;
  use deno_core::serde_json;
  use deno_core::v8;
  use deno_core::v8::ValueDeserializerHelper;
  use deno_core::v8::ValueSerializerHelper;
  use deno_error::JsErrorBox;
  pub use deno_process::ipc::INITIAL_CAPACITY;
  use deno_process::ipc::IpcAdvancedStreamError;
  use deno_process::ipc::IpcAdvancedStreamResource;
  use deno_process::ipc::IpcJsonStreamError;
  pub use deno_process::ipc::IpcJsonStreamResource;
  pub use deno_process::ipc::IpcRefTracker;
  use serde::Serialize;

  use crate::ChildPipeFd;
  use crate::ops::ipc::ChildIpcSerialization;

  /// Wrapper around v8 value that implements Serialize.
  struct SerializeWrapper<'a, 'b, 'c>(
    RefCell<&'b mut v8::PinScope<'a, 'c>>,
    v8::Local<'a, v8::Value>,
  );

  impl Serialize for SerializeWrapper<'_, '_, '_> {
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
    scope: &mut v8::PinScope<'a, '_>,
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
      if let Some(to_json) = object.get(scope, to_json_key)
        && let Ok(to_json) = to_json.try_cast::<v8::Function>()
      {
        let json_value = to_json.call(scope, object.into(), &[]).unwrap();
        return serialize_v8_value(scope, json_value, ser);
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
  #[to_v8]
  pub fn op_node_child_ipc_pipe(
    state: &mut OpState,
  ) -> Result<Option<(ResourceId, u8)>, io::Error> {
    let (fd, serialization) = match state.try_borrow_mut::<crate::ChildPipeFd>()
    {
      Some(ChildPipeFd(fd, serialization)) => (*fd, *serialization),
      None => return Ok(None),
    };
    log::debug!("op_node_child_ipc_pipe: {:?}, {:?}", fd, serialization);
    let ref_tracker = IpcRefTracker::new(state.external_ops_tracker.clone());
    match serialization {
      ChildIpcSerialization::Json => Ok(Some((
        state
          .resource_table
          .add(IpcJsonStreamResource::new(fd, ref_tracker)?),
        0,
      ))),
      ChildIpcSerialization::Advanced => Ok(Some((
        state
          .resource_table
          .add(IpcAdvancedStreamResource::new(fd, ref_tracker)?),
        1,
      ))),
    }
  }

  #[derive(Debug, thiserror::Error, deno_error::JsError)]
  pub enum IpcError {
    #[class(inherit)]
    #[error(transparent)]
    Resource(#[from] deno_core::error::ResourceError),
    #[class(inherit)]
    #[error(transparent)]
    IpcAdvancedStream(#[from] IpcAdvancedStreamError),
    #[class(inherit)]
    #[error(transparent)]
    IpcJsonStream(#[from] IpcJsonStreamError),
    #[class(inherit)]
    #[error(transparent)]
    Canceled(#[from] deno_core::Canceled),
    #[class(inherit)]
    #[error("failed to serialize json value: {0}")]
    SerdeJson(serde_json::Error),
    #[class(type)]
    #[error("Failed to read header")]
    ReadHeaderFailed,
    #[class(type)]
    #[error("Failed to read value")]
    ReadValueFailed,
  }

  #[op2(async)]
  pub fn op_node_ipc_write_json<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
    value: v8::Local<'a, v8::Value>,
    // using an array as an "out parameter".
    // index 0 is a boolean indicating whether the queue is under the limit.
    //
    // ideally we would just return `Result<(impl Future, bool), ..>`, but that's not
    // supported by `op2` currently.
    queue_ok: v8::Local<'a, v8::Array>,
  ) -> Result<impl Future<Output = Result<(), io::Error>> + use<>, IpcError> {
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

  pub struct AdvancedSerializerDelegate {
    constants: AdvancedIpcConstants,
  }

  impl AdvancedSerializerDelegate {
    fn new(constants: AdvancedIpcConstants) -> Self {
      Self { constants }
    }
  }

  const ARRAY_BUFFER_VIEW_TAG: u32 = 0;
  const NOT_ARRAY_BUFFER_VIEW_TAG: u32 = 1;

  fn ab_view_to_index<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    view: v8::Local<'s, v8::ArrayBufferView>,
    constants: &AdvancedIpcConstants,
  ) -> Option<u32> {
    if view.is_int8_array() {
      Some(0)
    } else if view.is_uint8_array() {
      let constructor = view
        .get(
          scope,
          v8::Local::new(scope, &constants.inner.constructor_key).into(),
        )
        .unwrap();
      let buffer_constructor = v8::Local::<v8::Value>::from(v8::Local::new(
        scope,
        &constants.inner.buffer_constructor,
      ));
      if constructor == buffer_constructor {
        Some(10)
      } else {
        Some(1)
      }
    } else if view.is_uint8_clamped_array() {
      Some(2)
    } else if view.is_int16_array() {
      Some(3)
    } else if view.is_uint16_array() {
      Some(4)
    } else if view.is_int32_array() {
      Some(5)
    } else if view.is_uint32_array() {
      Some(6)
    } else if view.is_float32_array() {
      Some(7)
    } else if view.is_float64_array() {
      Some(8)
    } else if view.is_data_view() {
      Some(9)
    } else if view.is_big_int64_array() {
      Some(11)
    } else if view.is_big_uint64_array() {
      Some(12)
    } else if view.is_float16_array() {
      Some(13)
    } else {
      None
    }
  }

  impl v8::ValueSerializerImpl for AdvancedSerializerDelegate {
    fn throw_data_clone_error<'s>(
      &self,
      scope: &mut v8::PinScope<'s, '_>,
      message: v8::Local<'s, v8::String>,
    ) {
      let error = v8::Exception::type_error(scope, message);
      scope.throw_exception(error);
    }

    fn has_custom_host_object(&self, _isolate: &v8::Isolate) -> bool {
      false
    }

    fn write_host_object<'s>(
      &self,
      scope: &mut v8::PinScope<'s, '_>,
      object: v8::Local<'s, v8::Object>,
      value_serializer: &dyn v8::ValueSerializerHelper,
    ) -> Option<bool> {
      if object.is_array_buffer_view() {
        let ab_view = object.cast::<v8::ArrayBufferView>();
        value_serializer.write_uint32(ARRAY_BUFFER_VIEW_TAG);
        let Some(index) = ab_view_to_index(scope, ab_view, &self.constants)
        else {
          scope.throw_exception(v8::Exception::type_error(
            scope,
            v8::String::new_from_utf8(
              scope,
              format!("Unserializable host object: {}", object.type_repr())
                .as_bytes(),
              v8::NewStringType::Normal,
            )
            .unwrap(),
          ));
          return None;
        };
        value_serializer.write_uint32(index);
        value_serializer.write_uint32(ab_view.byte_length() as u32);
        let mut storage = [0u8; v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP];
        let slice = ab_view.get_contents(&mut storage);
        value_serializer.write_raw_bytes(slice);
        Some(true)
      } else {
        value_serializer.write_uint32(NOT_ARRAY_BUFFER_VIEW_TAG);
        value_serializer
          .write_value(scope.get_current_context(), object.into());
        Some(true)
      }
    }

    fn get_shared_array_buffer_id<'s>(
      &self,
      _scope: &mut v8::PinScope<'s, '_>,
      _shared_array_buffer: v8::Local<'s, v8::SharedArrayBuffer>,
    ) -> Option<u32> {
      None
    }
  }

  #[derive(Clone)]
  struct AdvancedIpcConstants {
    inner: Rc<AdvancedIpcConstantsInner>,
  }
  struct AdvancedIpcConstantsInner {
    buffer_constructor: v8::Global<v8::Function>,
    constructor_key: v8::Global<v8::String>,
    fast_buffer_prototype: v8::Global<v8::Object>,
  }

  #[op2(fast)]
  pub fn op_node_ipc_buffer_constructor(
    scope: &mut v8::PinScope<'_, '_>,
    state: &mut OpState,
    buffer_constructor: v8::Local<'_, v8::Function>,
    fast_buffer_prototype: v8::Local<'_, v8::Object>,
  ) {
    if state.has::<AdvancedIpcConstants>() {
      return;
    }
    let constants = AdvancedIpcConstants {
      inner: Rc::new(AdvancedIpcConstantsInner {
        buffer_constructor: v8::Global::new(scope, buffer_constructor),
        constructor_key: v8::Global::new(
          scope,
          v8::String::new_from_utf8(
            scope,
            b"constructor",
            v8::NewStringType::Internalized,
          )
          .unwrap(),
        ),
        fast_buffer_prototype: v8::Global::new(scope, fast_buffer_prototype),
      }),
    };
    state.put(constants);
  }

  #[op2(async)]
  pub fn op_node_ipc_write_advanced<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
    value: v8::Local<'a, v8::Value>,
    // using an array as an "out parameter".
    // index 0 is a boolean indicating whether the queue is under the limit.
    //
    // ideally we would just return `Result<(impl Future, bool), ..>`, but that's not
    // supported by `op2` currently.
    queue_ok: v8::Local<'a, v8::Array>,
  ) -> Result<impl Future<Output = Result<(), io::Error>> + use<>, IpcError> {
    let constants = state.borrow().borrow::<AdvancedIpcConstants>().clone();
    let serializer = AdvancedSerializer::new(scope, constants);
    let serialized = serializer.serialize(scope, value)?;

    let stream = state
      .borrow()
      .resource_table
      .get::<IpcAdvancedStreamResource>(rid)?;
    let old = stream
      .queued_bytes
      .fetch_add(serialized.len(), std::sync::atomic::Ordering::Relaxed);
    if old + serialized.len() > 2 * INITIAL_CAPACITY {
      // sending messages too fast
      let Ok(v) = false.to_v8(scope);
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

  struct AdvancedSerializer {
    inner: v8::ValueSerializer<'static>,
  }

  impl AdvancedSerializer {
    fn new(
      scope: &mut v8::PinScope<'_, '_>,
      constants: AdvancedIpcConstants,
    ) -> Self {
      let inner = v8::ValueSerializer::new(
        scope,
        Box::new(AdvancedSerializerDelegate::new(constants)),
      );
      inner.set_treat_array_buffer_views_as_host_objects(true);
      Self { inner }
    }

    fn serialize<'s, 'i>(
      &self,
      scope: &mut v8::PinScope<'s, 'i>,
      value: v8::Local<'s, v8::Value>,
    ) -> Result<Vec<u8>, IpcError> {
      self.inner.write_raw_bytes(&[0, 0, 0, 0]);
      self.inner.write_header();
      let context = scope.get_current_context();
      self.inner.write_value(context, value);
      let mut ser = self.inner.release();
      let length = ser.len() - 4;
      ser[0] = ((length >> 24) & 0xFF) as u8;
      ser[1] = ((length >> 16) & 0xFF) as u8;
      ser[2] = ((length >> 8) & 0xFF) as u8;
      ser[3] = (length & 0xFF) as u8;
      Ok(ser)
    }
  }

  struct AdvancedIpcDeserializer {
    inner: v8::ValueDeserializer<'static>,
  }

  struct AdvancedIpcDeserializerDelegate {
    constants: AdvancedIpcConstants,
  }

  impl v8::ValueDeserializerImpl for AdvancedIpcDeserializerDelegate {
    fn read_host_object<'s>(
      &self,
      scope: &mut v8::PinScope<'s, '_>,
      deser: &dyn ValueDeserializerHelper,
    ) -> Option<v8::Local<'s, v8::Object>> {
      let throw_error = |message: &str| {
        scope.throw_exception(v8::Exception::type_error(
          scope,
          v8::String::new_from_utf8(
            scope,
            message.as_bytes(),
            v8::NewStringType::Normal,
          )
          .unwrap(),
        ));
        None
      };
      let mut tag = 0;
      if !deser.read_uint32(&mut tag) {
        return throw_error("Failed to read tag");
      }
      match tag {
        ARRAY_BUFFER_VIEW_TAG => {
          let mut index = 0;
          if !deser.read_uint32(&mut index) {
            return throw_error("Failed to read array buffer view type tag");
          }
          let mut byte_length = 0;
          if !deser.read_uint32(&mut byte_length) {
            return throw_error("Failed to read byte length");
          }
          let Some(buf) = deser.read_raw_bytes(byte_length as usize) else {
            return throw_error("failed to read bytes for typed array");
          };

          let array_buffer = v8::ArrayBuffer::new(scope, byte_length as usize);
          // SAFETY: array_buffer is valid as v8 is keeping it alive, and is byte_length bytes
          // buf is also byte_length bytes long
          unsafe {
            std::ptr::copy(
              buf.as_ptr(),
              array_buffer.data().unwrap().as_ptr().cast::<u8>(),
              byte_length as usize,
            );
          }

          let value = match index {
            0 => {
              v8::Int8Array::new(scope, array_buffer, 0, byte_length as usize)
                .unwrap()
                .into()
            }
            1 => {
              v8::Uint8Array::new(scope, array_buffer, 0, byte_length as usize)
                .unwrap()
                .into()
            }
            10 => {
              let obj: v8::Local<v8::Object> = v8::Uint8Array::new(
                scope,
                array_buffer,
                0,
                byte_length as usize,
              )?
              .into();
              let fast_proto = v8::Local::new(
                scope,
                &self.constants.inner.fast_buffer_prototype,
              );
              obj.set_prototype(scope, fast_proto.into());
              obj
            }
            2 => v8::Uint8ClampedArray::new(
              scope,
              array_buffer,
              0,
              byte_length as usize,
            )?
            .into(),
            3 => v8::Int16Array::new(
              scope,
              array_buffer,
              0,
              byte_length as usize / 2,
            )?
            .into(),
            4 => v8::Uint16Array::new(
              scope,
              array_buffer,
              0,
              byte_length as usize / 2,
            )?
            .into(),
            5 => v8::Int32Array::new(
              scope,
              array_buffer,
              0,
              byte_length as usize / 4,
            )?
            .into(),
            6 => v8::Uint32Array::new(
              scope,
              array_buffer,
              0,
              byte_length as usize / 4,
            )?
            .into(),
            7 => v8::Float32Array::new(
              scope,
              array_buffer,
              0,
              byte_length as usize / 4,
            )
            .unwrap()
            .into(),
            8 => v8::Float64Array::new(
              scope,
              array_buffer,
              0,
              byte_length as usize / 8,
            )?
            .into(),
            9 => {
              v8::DataView::new(scope, array_buffer, 0, byte_length as usize)
                .into()
            }
            11 => v8::BigInt64Array::new(
              scope,
              array_buffer,
              0,
              byte_length as usize / 8,
            )?
            .into(),
            12 => v8::BigUint64Array::new(
              scope,
              array_buffer,
              0,
              byte_length as usize / 8,
            )?
            .into(),
            // TODO(nathanwhit): this should just be `into()`, but I forgot to impl it in rusty_v8.
            // the underlying impl is just a transmute though.
            // SAFETY: float16array is an object
            13 => unsafe {
              std::mem::transmute::<
                v8::Local<v8::Float16Array>,
                v8::Local<v8::Object>,
              >(v8::Float16Array::new(
                scope,
                array_buffer,
                0,
                byte_length as usize / 2,
              )?)
            },
            _ => return None,
          };
          Some(value)
        }
        NOT_ARRAY_BUFFER_VIEW_TAG => {
          let value = deser.read_value(scope.get_current_context());
          Some(value.unwrap_or_else(|| v8::null(scope).into()).cast())
        }
        _ => {
          throw_error(&format!("Invalid tag: {}", tag));
          None
        }
      }
    }
  }

  impl AdvancedIpcDeserializer {
    fn new(
      scope: &mut v8::PinScope<'_, '_>,
      constants: AdvancedIpcConstants,
      msg_bytes: &[u8],
    ) -> Self {
      let inner = v8::ValueDeserializer::new(
        scope,
        Box::new(AdvancedIpcDeserializerDelegate { constants }),
        msg_bytes,
      );
      Self { inner }
    }
  }

  struct AdvancedIpcReadResult {
    msg_bytes: Option<Vec<u8>>,
    constants: AdvancedIpcConstants,
  }

  fn make_stop_sentinel<'s>(
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Value> {
    let obj = v8::Object::new(scope);
    obj.set(
      scope,
      v8::String::new_from_utf8(scope, b"cmd", v8::NewStringType::Internalized)
        .unwrap()
        .into(),
      v8::String::new_from_utf8(
        scope,
        b"NODE_CLOSE",
        v8::NewStringType::Internalized,
      )
      .unwrap()
      .into(),
    );
    obj.into()
  }

  impl<'a> deno_core::ToV8<'a> for AdvancedIpcReadResult {
    type Error = IpcError;
    fn to_v8(
      self,
      scope: &mut v8::PinScope<'a, '_>,
    ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
      let Some(msg_bytes) = self.msg_bytes else {
        return Ok(make_stop_sentinel(scope));
      };
      let deser =
        AdvancedIpcDeserializer::new(scope, self.constants, &msg_bytes);
      let context = scope.get_current_context();
      let header_success = deser.inner.read_header(context).unwrap_or(false);
      if !header_success {
        return Err(IpcError::ReadHeaderFailed);
      }
      let Some(value) = deser.inner.read_value(context) else {
        return Err(IpcError::ReadValueFailed);
      };
      Ok(value)
    }
  }

  #[op2(async)]
  #[to_v8]
  pub async fn op_node_ipc_read_advanced(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
  ) -> Result<AdvancedIpcReadResult, IpcError> {
    let stream = state
      .borrow()
      .resource_table
      .get::<IpcAdvancedStreamResource>(rid)?;
    let cancel = stream.cancel.clone();
    let mut stream = RcRef::map(stream, |r| &r.read_half).borrow_mut().await;
    let msg_bytes = stream.read_msg_bytes().or_cancel(cancel).await??;

    Ok(AdvancedIpcReadResult {
      msg_bytes,
      constants: state.borrow().borrow::<AdvancedIpcConstants>().clone(),
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
  pub async fn op_node_ipc_read_json(
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
  pub fn op_node_ipc_ref(
    state: &mut OpState,
    #[smi] rid: ResourceId,
    serialization_json: bool,
  ) {
    if serialization_json {
      let stream = state
        .resource_table
        .get::<IpcJsonStreamResource>(rid)
        .expect("Invalid resource ID");
      stream.ref_tracker.ref_();
    } else {
      let stream = state
        .resource_table
        .get::<IpcAdvancedStreamResource>(rid)
        .expect("Invalid resource ID");
      stream.ref_tracker.ref_();
    }
  }

  #[op2(fast)]
  pub fn op_node_ipc_unref(
    state: &mut OpState,
    #[smi] rid: ResourceId,
    serialization_json: bool,
  ) {
    if serialization_json {
      let stream = state
        .resource_table
        .get::<IpcJsonStreamResource>(rid)
        .expect("Invalid resource ID");
      stream.ref_tracker.unref();
    } else {
      let stream = state
        .resource_table
        .get::<IpcAdvancedStreamResource>(rid)
        .expect("Invalid resource ID");
      stream.ref_tracker.unref();
    }
  }

  #[cfg(test)]
  mod tests {
    use deno_core::JsRuntime;
    use deno_core::RuntimeOptions;
    use deno_core::v8;

    fn wrap_expr(s: &str) -> String {
      format!("(function () {{ return {s}; }})()")
    }

    fn serialize_js_to_json(runtime: &mut JsRuntime, js: String) -> String {
      let val = runtime.execute_script("", js).unwrap();
      deno_core::scope!(scope, runtime);
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
