// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::io::{StreamResource, StreamResourceHolder};
use crate::http_util::{create_http_client, HttpBody};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::*;
use futures::future::FutureExt;
use http::header::HeaderName;
use http::header::HeaderValue;
use http::Method;
use std;
use std::convert::From;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("op_fetch", s.stateful_json_op(op_fetch));
}

#[derive(Deserialize)]
struct FetchArgs {
  method: Option<String>,
  url: String,
  headers: Vec<(String, String)>,
}

pub fn op_fetch(
  state: &State,
  args: Value,
  data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: FetchArgs = serde_json::from_value(args)?;
  let url = args.url;

  let client =
    create_http_client(state.borrow().global_state.flags.ca_file.clone())?;

  let method = match args.method {
    Some(method_str) => Method::from_bytes(method_str.as_bytes())
      .map_err(|e| OpError::other(e.to_string()))?,
    None => Method::GET,
  };

  let url_ = url::Url::parse(&url).map_err(OpError::from)?;

  // Check scheme before asking for net permission
  let scheme = url_.scheme();
  if scheme != "http" && scheme != "https" {
    return Err(OpError::type_error(format!(
      "scheme '{}' not supported",
      scheme
    )));
  }

  state.check_net_url(&url_)?;

  let mut request = client.request(method, url_);

  if let Some(buf) = data {
    request = request.body(Vec::from(&*buf));
  }

  for (key, value) in args.headers {
    let name = HeaderName::from_bytes(key.as_bytes()).unwrap();
    let v = HeaderValue::from_str(&value).unwrap();
    request = request.header(name, v);
  }
  debug!("Before fetch {}", url);
  let state_ = state.clone();

  let future = async move {
    let res = request.send().await?;
    debug!("Fetch response {}", url);
    let status = res.status();
    let mut res_headers = Vec::new();
    for (key, val) in res.headers().iter() {
      res_headers.push((key.to_string(), val.to_str().unwrap().to_owned()));
    }

    let body = HttpBody::from(res);
    let mut state = state_.borrow_mut();
    let rid = state.resource_table.add(
      "httpBody",
      Box::new(StreamResourceHolder::new(StreamResource::HttpBody(
        Box::new(body),
      ))),
    );

    let json_res = json!({
      "bodyRid": rid,
      "status": status.as_u16(),
      "statusText": status.canonical_reason().unwrap_or(""),
      "headers": res_headers
    });

    Ok(json_res)
  };

  Ok(JsonOp::Async(future.boxed_local()))
}
