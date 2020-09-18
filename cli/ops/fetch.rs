// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::http_util::create_http_client;
use deno_core::error::bad_resource_id;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::url;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use http::header::HeaderName;
use http::header::HeaderValue;
use http::Method;
use reqwest::Client;
use reqwest::Response;
use serde::Deserialize;
use serde_json::Value;
use std::cell::RefCell;
use std::convert::From;
use std::path::PathBuf;
use std::rc::Rc;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_async(rt, "op_fetch", op_fetch);
  super::reg_json_async(rt, "op_fetch_read", op_fetch_read);
  super::reg_json_sync(rt, "op_create_http_client", op_create_http_client);
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
  state: Rc<RefCell<OpState>>,
  args: Value,
  data: BufVec,
) -> Result<Value, AnyError> {
  let args: FetchArgs = serde_json::from_value(args)?;
  let url = args.url;

  let client = if let Some(rid) = args.client_rid {
    let state = state.borrow();
    let r = state
      .resource_table
      .get::<HttpClientResource>(rid)
      .ok_or_else(bad_resource_id)?;
    r.client.clone()
  } else {
    let state_ = state.borrow();
    let client = state_.borrow::<reqwest::Client>();
    client.clone()
  };

  let method = match args.method {
    Some(method_str) => Method::from_bytes(method_str.as_bytes())?,
    None => Method::GET,
  };

  let url_ = url::Url::parse(&url)?;

  // Check scheme before asking for net permission
  let scheme = url_.scheme();
  if scheme != "http" && scheme != "https" {
    return Err(type_error(format!("scheme '{}' not supported", scheme)));
  }

  super::cli_state2(&state).check_net_url(&url_)?;

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

  let rid = state
    .borrow_mut()
    .resource_table
    .add("httpBody", Box::new(res));

  Ok(json!({
    "bodyRid": rid,
    "status": status.as_u16(),
    "statusText": status.canonical_reason().unwrap_or(""),
    "headers": res_headers
  }))
}

async fn op_fetch_read(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _data: BufVec,
) -> Result<Value, AnyError> {
  #[derive(Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct Args {
    rid: u32,
  }

  let args: Args = serde_json::from_value(args)?;
  let rid = args.rid;

  use futures::future::poll_fn;
  use futures::ready;
  use futures::FutureExt;
  let f = poll_fn(move |cx| {
    let mut state = state.borrow_mut();
    let response = state
      .resource_table
      .get_mut::<Response>(rid as u32)
      .ok_or_else(bad_resource_id)?;

    let mut chunk_fut = response.chunk().boxed_local();
    let r = ready!(chunk_fut.poll_unpin(cx))?;
    if let Some(chunk) = r {
      Ok(json!({ "chunk": &*chunk })).into()
    } else {
      Ok(json!({ "chunk": null })).into()
    }
  });
  f.await
  /*
  // I'm programming this as I want it to be programmed, even though it might be
  // incorrect, normally we would use poll_fn here. We need to make this await pattern work.
  let chunk = response.chunk().await?;
  if let Some(chunk) = chunk {
    // TODO(ry) This is terribly inefficient. Make this zero-copy.
    Ok(json!({ "chunk": &*chunk }))
  } else {
    Ok(json!({ "chunk": null }))
  }
  */
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
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateHttpClientOptions = serde_json::from_value(args)?;

  if let Some(ca_file) = args.ca_file.clone() {
    super::cli_state(state).check_read(&PathBuf::from(ca_file))?;
  }

  let client = create_http_client(args.ca_file.as_deref()).unwrap();

  let rid = state
    .resource_table
    .add("httpClient", Box::new(HttpClientResource::new(client)));
  Ok(json!(rid))
}
