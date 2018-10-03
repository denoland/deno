// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Helpers for serialization.
use errors::DenoError;
use flatbuffers;
use hyper::Body;
use hyper::Request;
use isolate::Buf;
use msg;

// Response to individual flatbuffer message, not a response in the HTTP sense.
pub fn serialize_response(
  cmd_id: u32,
  builder: &mut flatbuffers::FlatBufferBuilder,
  mut args: msg::BaseArgs,
) -> Buf {
  args.cmd_id = cmd_id;
  let base = msg::Base::create(builder, &args);
  msg::finish_base_buffer(builder, base);
  let data = builder.finished_data();
  // println!("serialize_response {:x?}", data);
  let vec = data.to_vec();
  vec.into_boxed_slice()
}

pub fn serialize_error<'bldr>(cmd_id: u32, err: DenoError) -> Buf {
  let builder = &mut flatbuffers::FlatBufferBuilder::new();
  let errmsg_offset = builder.create_string(&format!("{}", err));
  serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      error: Some(errmsg_offset),
      error_kind: err.kind(),
      ..Default::default()
    },
  )
}

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

pub fn serialize_request_header<'bldr>(
  builder: &mut flatbuffers::FlatBufferBuilder<'bldr>,
  r: &Request<Body>,
) -> flatbuffers::WIPOffset<msg::HttpHeader<'bldr>> {
  let method = builder.create_string(r.method().as_str());
  let url = builder.create_string(r.uri().to_string().as_ref());

  let mut fields = Vec::new();
  for (key, val) in r.headers().iter() {
    let kv = serialize_key_value(builder, key.as_ref(), val.to_str().unwrap());
    fields.push(kv);
  }
  let fields = builder.create_vector(fields.as_ref());

  msg::HttpHeader::create(
    builder,
    &msg::HttpHeaderArgs {
      is_request: true,
      method: Some(method),
      url: Some(url),
      fields: Some(fields),
      ..Default::default()
    },
  )
}
