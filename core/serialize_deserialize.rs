// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use rusty_v8 as v8;

type ArrayBuffers = Vec<v8::SharedRef<v8::BackingStore>>;

struct SerializeDeserialize<'a> {
  array_buffers: &'a mut ArrayBuffers,
}

impl<'a> SerializeDeserialize<'a> {
  fn serializer<'s>(
    scope: &mut v8::HandleScope<'s>,
    array_buffers: &'a mut ArrayBuffers,
  ) -> v8::ValueSerializer<'a, 's> {
    v8::ValueSerializer::new(scope, Box::new(Self { array_buffers }))
  }

  fn deserializer<'s>(
    scope: &mut v8::HandleScope<'s>,
    data: &[u8],
    array_buffers: &'a mut ArrayBuffers,
  ) -> v8::ValueDeserializer<'a, 's> {
    v8::ValueDeserializer::new(scope, Box::new(Self { array_buffers }), data)
  }
}

impl<'a> v8::ValueSerializerImpl for SerializeDeserialize<'a> {
  #[allow(unused_variables)]
  fn throw_data_clone_error<'s>(
    &mut self,
    scope: &mut v8::HandleScope<'s>,
    message: v8::Local<'s, v8::String>,
  ) {
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
  }

  #[allow(unused_variables)]
  fn get_shared_array_buffer_id<'s>(
    &mut self,
    scope: &mut v8::HandleScope<'s>,
    shared_array_buffer: v8::Local<'s, v8::SharedArrayBuffer>,
  ) -> Option<u32> {
    self
      .array_buffers
      .push(v8::SharedArrayBuffer::get_backing_store(
        &shared_array_buffer,
      ));
    Some((self.array_buffers.len() as u32) - 1)
  }
}

impl<'a> v8::ValueDeserializerImpl for SerializeDeserialize<'a> {
  #[allow(unused_variables)]
  fn get_shared_array_buffer_from_id<'s>(
    &mut self,
    scope: &mut v8::HandleScope<'s>,
    transfer_id: u32,
  ) -> Option<v8::Local<'s, v8::SharedArrayBuffer>> {
    let backing_store = self.array_buffers.get(transfer_id as usize).unwrap();
    Some(v8::SharedArrayBuffer::with_backing_store(
      scope,
      backing_store,
    ))
  }
}

pub(crate) fn serialize(
  scope: &mut v8::HandleScope,
  obj: v8::Local<v8::Value>,
) -> Option<Vec<u8>> {
  let mut array_buffers = ArrayBuffers::new(); // TODO(Inteon): fix array buffers
  let mut value_serializer =
    SerializeDeserialize::serializer(scope, &mut array_buffers);
  match value_serializer.write_value(scope.get_current_context(), obj) {
    Some(true) => Some(value_serializer.release()),
    _ => None,
  }
}

pub(crate) fn deserialize<'s>(
  scope: &'s mut v8::HandleScope,
  buffer: &[u8],
) -> Option<v8::Local<'s, v8::Value>> {
  let mut array_buffers = ArrayBuffers::new(); // TODO(Inteon): fix array buffers
  let mut value_deserializer =
    SerializeDeserialize::deserializer(scope, &buffer, &mut array_buffers);
  unsafe {
    core::mem::transmute(
      value_deserializer.read_value(scope.get_current_context()),
    ) // TODO(Inteon): fix lifetime bug in rusty_v8?
  }
}
