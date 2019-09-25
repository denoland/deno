// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::http_util::get_client;
use crate::resources;
use crate::state::ThreadSafeState;
use deno::*;
use http::header::HeaderName;
use http::header::HeaderValue;
use http::Method;
use hyper;
use hyper::rt::Future;
use std;
use std::convert::From;

#[derive(Deserialize)]
struct FetchArgs {
  method: Option<String>,
  url: String,
  headers: Vec<(String, String)>,
}

pub fn op_fetch(
  state: &ThreadSafeState,
  args: Value,
  data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: FetchArgs = serde_json::from_value(args)?;
  let url = args.url;

  let client = get_client();

  let method = match args.method {
    Some(method_str) => Method::from_bytes(method_str.as_bytes())?,
    None => Method::GET,
  };

  let url_ = url::Url::parse(&url).map_err(ErrBox::from)?;
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
  let future = request.send().map_err(ErrBox::from).and_then(move |res| {
    let status = res.status();
    let mut res_headers = Vec::new();
    for (key, val) in res.headers().iter() {
      res_headers.push((key.to_string(), val.to_str().unwrap().to_owned()));
    }

    let body = res.into_body();
    let body_resource = resources::add_reqwest_body(body);

    let json_res = json!({
      "bodyRid": body_resource.rid,
      "status": status.as_u16(),
      "statusText": status.canonical_reason().unwrap_or(""),
      "headers": res_headers
    });

    futures::future::ok(json_res)
  });

  Ok(JsonOp::Async(Box::new(future)))
}
