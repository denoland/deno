// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::http_util;
use crate::msg;
use crate::msg_util;
use crate::ops::serialize_response;
use crate::ops::CliOpResult;
use crate::resources;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use hyper;
use hyper::rt::Future;
use std;
use std::convert::From;

pub fn op_fetch(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  let inner = base.inner_as_fetch().unwrap();
  let cmd_id = base.cmd_id();

  let header = inner.header().unwrap();
  assert!(header.is_request());
  let url = header.url().unwrap();

  let body = match data {
    None => hyper::Body::empty(),
    Some(buf) => hyper::Body::from(Vec::from(&*buf)),
  };

  let req = msg_util::deserialize_request(header, body)?;

  let url_ = url::Url::parse(url).map_err(ErrBox::from)?;
  state.check_net_url(&url_)?;

  let client = http_util::get_client();

  debug!("Before fetch {}", url);
  let future = client
    .request(req)
    .map_err(ErrBox::from)
    .and_then(move |res| {
      let builder = &mut FlatBufferBuilder::new();
      let header_off = msg_util::serialize_http_response(builder, &res);
      let body = res.into_body();
      let body_resource = resources::add_hyper_body(body);
      let inner = msg::FetchRes::create(
        builder,
        &msg::FetchResArgs {
          header: Some(header_off),
          body_rid: body_resource.rid,
        },
      );

      Ok(serialize_response(
        cmd_id,
        builder,
        msg::BaseArgs {
          inner: Some(inner.as_union_value()),
          inner_type: msg::Any::FetchRes,
          ..Default::default()
        },
      ))
    });
  if base.sync() {
    let result_buf = future.wait()?;
    Ok(Op::Sync(result_buf))
  } else {
    Ok(Op::Async(Box::new(future)))
  }
}
