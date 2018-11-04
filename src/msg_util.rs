// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Helpers for serialization.
use flatbuffers;
use msg;

pub fn serialize_key_value<'bldr>(
  builder: &mut flatbuffers::FlatBufferBuilder<'bldr>,
  key: &str,
  value: &str,
) -> flatbuffers::WIPOffset<msg::KeyValue<'bldr>> {
  let key = builder.create_string(&key);
  let value = builder.create_string(&value);
  msg::KeyValue::create(
    builder,
    &msg::KeyValueArgs {
      key: Some(key),
      value: Some(value),
    },
  )
}
