// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::http_util;
use crate::resources;
use crate::state::ThreadSafeState;
use deno::*;
use http::header::HeaderName;
use http::uri::Uri;
use http::Method;
use hyper;
use hyper::header::HeaderValue;
use hyper::rt::Future;
use hyper::Request;
use std;
use std::convert::From;
use std::str::FromStr;

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

  let body = match data {
    None => hyper::Body::empty(),
    Some(buf) => hyper::Body::from(Vec::from(&*buf)),
  };

  let mut req = Request::new(body);
  let uri = Uri::from_str(&url).map_err(ErrBox::from)?;
  *req.uri_mut() = uri;

  if let Some(method) = args.method {
    let method = Method::from_str(&method).unwrap();
    *req.method_mut() = method;
  }

  let headers = req.headers_mut();
  for header_pair in args.headers {
    let name = HeaderName::from_bytes(header_pair.0.as_bytes()).unwrap();
    let v = HeaderValue::from_str(&header_pair.1).unwrap();
    headers.insert(name, v);
  }

  let url_ = url::Url::parse(&url).map_err(ErrBox::from)?;
  state.check_net_url(&url_)?;

  let client = http_util::get_client();

  debug!("Before fetch {}", url);
  let future = client
    .request(req)
    .map_err(ErrBox::from)
    .and_then(move |res| {
      let status = res.status().as_u16();
      let mut res_headers = Vec::new();
      for (key, val) in res.headers().iter() {
        res_headers.push((key.to_string(), val.to_str().unwrap().to_owned()));
      }
      let body = res.into_body();
      let body_resource = resources::add_hyper_body(body);

      let json_res = json!({
        "bodyRid": body_resource.rid,
        "status": status,
        "headers": res_headers
      });

      futures::future::ok(json_res)
    });

  Ok(JsonOp::Async(Box::new(future)))
}
