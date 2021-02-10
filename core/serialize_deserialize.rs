// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use rusty_v8 as v8;

struct SerializeDeserialize {}

impl SerializeDeserialize {
  fn serializer<'s>(
    scope: &mut v8::HandleScope<'s>,
  ) -> v8::ValueSerializer<'static, 's> {
    v8::ValueSerializer::new(scope, Box::new(Self { }))
  }

  fn deserializer<'s>(
    scope: &mut v8::HandleScope<'s>,
    data: &[u8],
  ) -> v8::ValueDeserializer<'static, 's> {
    v8::ValueDeserializer::new(scope, Box::new(Self { }), data)
  }
}

impl v8::ValueSerializerImpl for SerializeDeserialize {
  #[allow(unused_variables)]
  fn throw_data_clone_error<'s>(
    &mut self,
    scope: &mut v8::HandleScope<'s>,
    message: v8::Local<'s, v8::String>,
  ) {
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
  }
}

impl v8::ValueDeserializerImpl for SerializeDeserialize {}

pub(crate) fn serialize(
  scope: &mut v8::HandleScope,
  obj: v8::Local<v8::Value>,
) -> Option<Vec<u8>> {
  let mut value_serializer =
    SerializeDeserialize::serializer(scope);
  match value_serializer.write_value(scope.get_current_context(), obj) {
    Some(true) => Some(value_serializer.release()),
    _ => None,
  }
}

pub(crate) fn deserialize<'s>(
  scope: &'s mut v8::HandleScope,
  buffer: &[u8],
) -> Option<v8::Local<'s, v8::Value>> {
  let mut value_deserializer =
    SerializeDeserialize::deserializer(scope, &buffer);
  value_deserializer.read_value(scope.get_current_context())
}
