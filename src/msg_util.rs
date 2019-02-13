// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Helpers for serialization.
use crate::errors;
use crate::errors::DenoResult;
use crate::msg;

use flatbuffers;
use http::header::HeaderName;
use http::uri::Uri;
use http::Method;
use hyper::header::HeaderMap;
use hyper::header::HeaderValue;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use std::str::FromStr;

type Headers = HeaderMap<HeaderValue>;

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

pub fn serialize_fields<'bldr>(
  builder: &mut flatbuffers::FlatBufferBuilder<'bldr>,
  headers: &Headers,
) -> flatbuffers::WIPOffset<
  flatbuffers::Vector<
    'bldr,
    flatbuffers::ForwardsUOffset<msg::KeyValue<'bldr>>,
  >,
> {
  let mut fields = Vec::new();
  for (key, val) in headers.iter() {
    let kv = serialize_key_value(builder, key.as_ref(), val.to_str().unwrap());
    fields.push(kv);
  }
  builder.create_vector(fields.as_ref())
}

// Not to be confused with serialize_response which has nothing to do with HTTP.
pub fn serialize_http_response<'bldr>(
  builder: &mut flatbuffers::FlatBufferBuilder<'bldr>,
  r: &Response<Body>,
) -> flatbuffers::WIPOffset<msg::HttpHeader<'bldr>> {
  let status = r.status().as_u16();
  let fields = serialize_fields(builder, r.headers());
  msg::HttpHeader::create(
    builder,
    &msg::HttpHeaderArgs {
      is_request: false,
      status,
      fields: Some(fields),
      ..Default::default()
    },
  )
}

pub fn deserialize_request(
  header_msg: msg::HttpHeader<'_>,
  body: Body,
) -> DenoResult<Request<Body>> {
  let mut r = Request::new(body);

  assert!(header_msg.is_request());

  let u = header_msg.url().unwrap();
  let u = Uri::from_str(u)
    .map_err(|e| errors::new(msg::ErrorKind::InvalidUri, e.to_string()))?;
  *r.uri_mut() = u;

  if let Some(method) = header_msg.method() {
    let method = Method::from_str(method).unwrap();
    *r.method_mut() = method;
  }

  if let Some(fields) = header_msg.fields() {
    let headers = r.headers_mut();
    for i in 0..fields.len() {
      let kv = fields.get(i);
      let key = kv.key().unwrap();
      let name = HeaderName::from_bytes(key.as_bytes()).unwrap();
      let value = kv.value().unwrap();
      let v = HeaderValue::from_str(value).unwrap();
      headers.insert(name, v);
    }
  }
  Ok(r)
}
