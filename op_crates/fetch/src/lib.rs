// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// use super::io::StreamResource;
// use super::io::StreamResourceHolder;
// use crate::http_util::HttpBody;
use deno_core::error::bad_resource_id;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::Client;
use reqwest::Method;
use serde::Deserialize;
use std::cell::RefCell;
use std::convert::From;
use std::path::PathBuf;
use std::rc::Rc;

pub fn init_js(rt: &mut deno_core::JsRuntime) {
  use deno_core::js_check;
  use std::path::Path;
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let files = vec![
    manifest_dir.join("src/01_web_util.js"),
    manifest_dir.join("src/11_streams.js"),
    manifest_dir.join("src/20_blob.js"),
    manifest_dir.join("src/23_multipart.js"),
    manifest_dir.join("src/26_fetch.js"),
  ];
  let display_root = manifest_dir.parent().unwrap().parent().unwrap();
  for file in files {
    println!("cargo:rerun-if-changed={}", file.display());
    let display_path = file.strip_prefix(display_root).unwrap();
    let display_path_str = display_path.display().to_string();
    js_check(rt.execute(
      &("deno:".to_string() + &display_path_str.replace('\\', "/")),
      &std::fs::read_to_string(&file).unwrap(),
    ));
  }
}

pub fn init(rt: &mut deno_core::JsRuntime) {
  rt.register_op("op_fetch", deno_core::json_op_async(op_fetch));
  rt.register_op(
    "op_create_http_client",
    deno_core::json_op_sync(op_create_http_client),
  );
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

  // TODO super::cli_state2(&state).check_net_url(&url_)?;

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

  // debug!("Fetch response {}", url);
  let status = res.status();
  let mut res_headers = Vec::new();
  for (key, val) in res.headers().iter() {
    res_headers.push((key.to_string(), val.to_str().unwrap().to_owned()));
  }

  /*
  let body = HttpBody::from(res);
  let rid = state.borrow_mut().resource_table.add(
    "httpBody",
    Box::new(StreamResourceHolder::new(StreamResource::HttpBody(
      Box::new(body),
    ))),
  );
  */
  let rid = 123;

  Ok(json!({
    "bodyRid": rid,
    "status": status.as_u16(),
    "statusText": status.canonical_reason().unwrap_or(""),
    "headers": res_headers
  }))
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
    todo!()
    // super::cli_state(state).check_read(&PathBuf::from(ca_file))?;
  }

  let client = create_http_client(args.ca_file.as_deref()).unwrap();

  let rid = state
    .resource_table
    .add("httpClient", Box::new(HttpClientResource::new(client)));
  Ok(json!(rid))
}

fn create_http_client(ca_file: Option<&str>) -> Result<Client, AnyError> {
  todo!()
}
