// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

use deno_core::error::bad_resource_id;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url;
use deno_core::url::Url;
use deno_core::BufVec;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;

use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::redirect::Policy;
use reqwest::Client;
use reqwest::Method;
use reqwest::Response;
use serde::Deserialize;
use std::cell::RefCell;
use std::convert::From;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::rc::Rc;

pub use reqwest; // Re-export reqwest

/// Execute this crates' JS source files.
pub fn init(isolate: &mut JsRuntime) {
  let files = vec![
    (
      "deno:op_crates/fetch/01_fetch_util.js",
      include_str!("01_fetch_util.js"),
    ),
    (
      "deno:op_crates/fetch/03_dom_iterable.js",
      include_str!("03_dom_iterable.js"),
    ),
    (
      "deno:op_crates/fetch/11_streams.js",
      include_str!("11_streams.js"),
    ),
    (
      "deno:op_crates/fetch/20_headers.js",
      include_str!("20_headers.js"),
    ),
    (
      "deno:op_crates/fetch/26_fetch.js",
      include_str!("26_fetch.js"),
    ),
  ];
  for (url, source_code) in files {
    isolate.execute(url, source_code).unwrap();
  }
}

pub trait FetchPermissions {
  fn check_net_url(&self, _url: &Url) -> Result<(), AnyError>;
  fn check_read(&self, _p: &PathBuf) -> Result<(), AnyError>;
}

/// For use with `op_fetch` when the user does not want permissions.
pub struct NoFetchPermissions;

impl FetchPermissions for NoFetchPermissions {
  fn check_net_url(&self, _url: &Url) -> Result<(), AnyError> {
    Ok(())
  }

  fn check_read(&self, _p: &PathBuf) -> Result<(), AnyError> {
    Ok(())
  }
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_fetch.d.ts")
}

pub async fn op_fetch<FP>(
  state: Rc<RefCell<OpState>>,
  args: Value,
  data: BufVec,
) -> Result<Value, AnyError>
where
  FP: FetchPermissions + 'static,
{
  #[derive(Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct FetchArgs {
    method: Option<String>,
    url: String,
    headers: Vec<(String, String)>,
    client_rid: Option<u32>,
  }

  let args: FetchArgs = serde_json::from_value(args)?;
  let url = args.url;

  let client = if let Some(rid) = args.client_rid {
    let state_ = state.borrow();
    let r = state_
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

  {
    let state_ = state.borrow();
    let permissions = state_.borrow::<FP>();
    permissions.check_net_url(&url_)?;
  }

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
  //debug!("Before fetch {}", url);

  let res = request.send().await?;

  //debug!("Fetch response {}", url);
  let status = res.status();
  let mut res_headers = Vec::new();
  for (key, val) in res.headers().iter() {
    let key_string = key.to_string();

    if val.as_bytes().is_ascii() {
      res_headers.push((key_string, val.to_str().unwrap().to_owned()))
    } else {
      res_headers.push((
        key_string,
        val
          .as_bytes()
          .iter()
          .map(|&c| c as char)
          .collect::<String>(),
      ));
    }
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

pub async fn op_fetch_read(
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
      // TODO(ry) This is terribly inefficient. Make this zero-copy.
      Ok(json!({ "chunk": &*chunk })).into()
    } else {
      Ok(json!({ "chunk": null })).into()
    }
  });
  f.await
}

struct HttpClientResource {
  client: Client,
}

impl HttpClientResource {
  fn new(client: Client) -> Self {
    Self { client }
  }
}

pub fn op_create_http_client<FP>(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError>
where
  FP: FetchPermissions + 'static,
{
  #[derive(Deserialize, Default, Debug)]
  #[serde(rename_all = "camelCase")]
  #[serde(default)]
  struct CreateHttpClientOptions {
    ca_file: Option<String>,
    ca_data: Option<String>,
  }

  let args: CreateHttpClientOptions = serde_json::from_value(args)?;

  if let Some(ca_file) = args.ca_file.clone() {
    let permissions = state.borrow::<FP>();
    permissions.check_read(&PathBuf::from(ca_file))?;
  }

  let client =
    create_http_client(args.ca_file.as_deref(), args.ca_data.as_deref())
      .unwrap();

  let rid = state
    .resource_table
    .add("httpClient", Box::new(HttpClientResource::new(client)));
  Ok(json!(rid))
}

/// Create new instance of async reqwest::Client. This client supports
/// proxies and doesn't follow redirects.
fn create_http_client(
  ca_file: Option<&str>,
  ca_data: Option<&str>,
) -> Result<Client, AnyError> {
  let mut builder = Client::builder().redirect(Policy::none()).use_rustls_tls();
  if let Some(ca_file) = ca_file {
    let mut buf = Vec::new();
    File::open(ca_file)?.read_to_end(&mut buf)?;
    let cert = reqwest::Certificate::from_pem(&buf)?;
    builder = builder.add_root_certificate(cert);
  } else if let Some(ca_data) = ca_data {
    let ca_data_vec = ca_data.as_bytes().to_vec();
    let cert = reqwest::Certificate::from_pem(&ca_data_vec)?;
    builder = builder.add_root_certificate(cert);
  }
  builder
    .build()
    .map_err(|_| deno_core::error::generic_error("Unable to build http client"))
}
