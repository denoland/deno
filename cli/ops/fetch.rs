// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::io::{StreamResource, StreamResourceHolder};
use crate::http_util::{create_http_client, HttpBody};
use crate::state::State;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::OpRegistry;
use deno_core::ZeroCopyBuf;
use http::header::HeaderName;
use http::header::HeaderValue;
use http::Method;
use reqwest::Client;
use serde_derive::Deserialize;
use serde_json::Value;
use std::convert::From;
use std::path::PathBuf;
use std::rc::Rc;

pub fn init(s: &Rc<State>) {
  s.register_op_json_async("op_fetch", op_fetch);
  s.register_op_json_sync("op_create_http_client", op_create_http_client);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchArgs {
  method: Option<String>,
  url: String,
  headers: Vec<(String, String)>,
  client_rid: Option<u32>,
}

async fn op_fetch(
  state: Rc<State>,
  args: Value,
  data: BufVec,
) -> Result<Value, ErrBox> {
  let args: FetchArgs = serde_json::from_value(args)?;
  let url = args.url;

  let client = if let Some(rid) = args.client_rid {
    let resource_table_ = state.resource_table.borrow();
    let r = resource_table_
      .get::<HttpClientResource>(rid)
      .ok_or_else(ErrBox::bad_resource_id)?;
    r.client.clone()
  } else {
    let client_ref = state.http_client.borrow_mut();
    client_ref.clone()
  };

  let method = match args.method {
    Some(method_str) => Method::from_bytes(method_str.as_bytes())?,
    None => Method::GET,
  };

  let url_ = url::Url::parse(&url)?;

  // Check scheme before asking for net permission
  let scheme = url_.scheme();
  if scheme != "http" && scheme != "https" {
    return Err(ErrBox::type_error(format!(
      "scheme '{}' not supported",
      scheme
    )));
  }

  state.check_net_url(&url_)?;

  let mut request = client.request(method, url_);

  match data.len() {
    0 => {}
    1 => request = request.body(Vec::from(&*data[0])),
    _ => panic!("Invalid number of arguments"),
  }

  for (key, value) in args.headers {
    let name = HeaderName::from_bytes(key.as_bytes()).unwrap();
    let v = HeaderValue::from_str(&value).unwrap();
    request = request.header(name, v);
  }
  debug!("Before fetch {}", url);

  let res = request.send().await?;

  debug!("Fetch response {}", url);
  let status = res.status();
  let mut res_headers = Vec::new();
  for (key, val) in res.headers().iter() {
    res_headers.push((key.to_string(), val.to_str().unwrap().to_owned()));
  }

  let body = HttpBody::from(res);
  let rid = state.resource_table.borrow_mut().add(
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
}

struct HttpClientResource {
  client: Client,
}

impl HttpClientResource {
  fn new(client: Client) -> Self {
    Self { client }
  }
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
struct CreateHttpClientOptions {
  ca_file: Option<String>,
}

fn op_create_http_client(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: CreateHttpClientOptions = serde_json::from_value(args)?;

  if let Some(ca_file) = args.ca_file.clone() {
    state.check_read(&PathBuf::from(ca_file))?;
  }

  let client = create_http_client(args.ca_file.as_deref()).unwrap();

  let rid = state
    .resource_table
    .borrow_mut()
    .add("httpClient", Box::new(HttpClientResource::new(client)));
  Ok(json!(rid))
}
