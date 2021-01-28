// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

use deno_core::error::bad_resource_id;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::Future;
use deno_core::futures::Stream;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::AsyncRefCell;
use deno_core::BufVec;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;

use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::redirect::Policy;
use reqwest::Body;
use reqwest::Client;
use reqwest::Method;
use reqwest::Response;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::From;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::io::StreamReader;

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
  fn check_net_url(&mut self, _url: &Url) -> Result<(), AnyError>;
  fn check_read(&mut self, _p: &PathBuf) -> Result<(), AnyError>;
}

/// For use with `op_fetch` when the user does not want permissions.
pub struct NoFetchPermissions;

impl FetchPermissions for NoFetchPermissions {
  fn check_net_url(&mut self, _url: &Url) -> Result<(), AnyError> {
    Ok(())
  }

  fn check_read(&mut self, _p: &PathBuf) -> Result<(), AnyError> {
    Ok(())
  }
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_fetch.d.ts")
}

pub fn op_fetch<FP>(
  state: &mut OpState,
  args: Value,
  data: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError>
where
  FP: FetchPermissions + 'static,
{
  #[derive(Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct FetchArgs {
    method: Option<String>,
    url: String,
    base_url: Option<String>,
    headers: Vec<(String, String)>,
    client_rid: Option<u32>,
    has_body: bool,
  }

  let args: FetchArgs = serde_json::from_value(args)?;

  let client = if let Some(rid) = args.client_rid {
    let r = state
      .resource_table
      .get::<HttpClientResource>(rid)
      .ok_or_else(bad_resource_id)?;
    r.client.clone()
  } else {
    let client = state.borrow::<reqwest::Client>();
    client.clone()
  };

  let method = match args.method {
    Some(method_str) => Method::from_bytes(method_str.as_bytes())?,
    None => Method::GET,
  };

  let base_url = match args.base_url {
    Some(base_url) => Some(Url::parse(&base_url)?),
    _ => None,
  };
  let url = Url::options()
    .base_url(base_url.as_ref())
    .parse(&args.url)?;

  // Check scheme before asking for net permission
  let scheme = url.scheme();
  if scheme != "http" && scheme != "https" {
    return Err(type_error(format!("scheme '{}' not supported", scheme)));
  }

  let permissions = state.borrow_mut::<FP>();
  permissions.check_net_url(&url)?;

  let mut request = client.request(method, url);

  let maybe_request_body_rid = if args.has_body {
    match data.len() {
      0 => {
        // If no body is passed, we return a writer for streaming the body.
        let (tx, rx) = mpsc::channel::<std::io::Result<Vec<u8>>>(1);
        request = request.body(Body::wrap_stream(ReceiverStream::new(rx)));

        let request_body_rid =
          state.resource_table.add(FetchRequestBodyResource {
            body: AsyncRefCell::new(tx),
            cancel: CancelHandle::default(),
          });

        Some(request_body_rid)
      }
      1 => {
        // If a body is passed, we use it, and don't return a body for streaming.
        request = request.body(Vec::from(&*data[0]));
        None
      }
      _ => panic!("Invalid number of arguments"),
    }
  } else {
    None
  };

  for (key, value) in args.headers {
    let name = HeaderName::from_bytes(key.as_bytes()).unwrap();
    let v = HeaderValue::from_str(&value).unwrap();
    request = request.header(name, v);
  }

  let fut = request.send();

  let request_rid = state
    .resource_table
    .add(FetchRequestResource(Box::pin(fut)));

  Ok(json!({
    "requestRid": request_rid,
    "requestBodyRid": maybe_request_body_rid
  }))
}

pub async fn op_fetch_send(
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

  let request = state
    .borrow_mut()
    .resource_table
    .take::<FetchRequestResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let request = Rc::try_unwrap(request)
    .ok()
    .expect("multiple op_fetch_send ongoing");

  let res = match request.0.await {
    Ok(res) => res,
    Err(e) => return Err(type_error(e.to_string())),
  };

  //debug!("Fetch response {}", url);
  let status = res.status();
  let url = res.url().to_string();
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

  let stream: BytesStream = Box::pin(res.bytes_stream().map(|r| {
    r.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
  }));
  let stream_reader = StreamReader::new(stream);
  let rid = state
    .borrow_mut()
    .resource_table
    .add(FetchResponseBodyResource {
      reader: AsyncRefCell::new(stream_reader),
      cancel: CancelHandle::default(),
    });

  Ok(json!({
    "status": status.as_u16(),
    "statusText": status.canonical_reason().unwrap_or(""),
    "headers": res_headers,
    "url": url,
    "responseRid": rid,
  }))
}

pub async fn op_fetch_request_write(
  state: Rc<RefCell<OpState>>,
  args: Value,
  data: BufVec,
) -> Result<Value, AnyError> {
  #[derive(Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct Args {
    rid: u32,
  }

  let args: Args = serde_json::from_value(args)?;
  let rid = args.rid;

  let buf = match data.len() {
    1 => Vec::from(&*data[0]),
    _ => panic!("Invalid number of arguments"),
  };

  let resource = state
    .borrow()
    .resource_table
    .get::<FetchRequestBodyResource>(rid as u32)
    .ok_or_else(bad_resource_id)?;
  let body = RcRef::map(&resource, |r| &r.body).borrow_mut().await;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  body.send(Ok(buf)).or_cancel(cancel).await??;

  Ok(json!({}))
}

pub async fn op_fetch_response_read(
  state: Rc<RefCell<OpState>>,
  args: Value,
  data: BufVec,
) -> Result<Value, AnyError> {
  #[derive(Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct Args {
    rid: u32,
  }

  let args: Args = serde_json::from_value(args)?;
  let rid = args.rid;

  if data.len() != 1 {
    panic!("Invalid number of arguments");
  }

  let resource = state
    .borrow()
    .resource_table
    .get::<FetchResponseBodyResource>(rid as u32)
    .ok_or_else(bad_resource_id)?;
  let mut reader = RcRef::map(&resource, |r| &r.reader).borrow_mut().await;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let mut buf = data[0].clone();
  let read = reader.read(&mut buf).try_or_cancel(cancel).await?;
  Ok(json!({ "read": read }))
}

struct FetchRequestResource(
  Pin<Box<dyn Future<Output = Result<Response, reqwest::Error>>>>,
);

impl Resource for FetchRequestResource {
  fn name(&self) -> Cow<str> {
    "fetchRequest".into()
  }
}

struct FetchRequestBodyResource {
  body: AsyncRefCell<mpsc::Sender<std::io::Result<Vec<u8>>>>,
  cancel: CancelHandle,
}

impl Resource for FetchRequestBodyResource {
  fn name(&self) -> Cow<str> {
    "fetchRequestBody".into()
  }
}

type BytesStream =
  Pin<Box<dyn Stream<Item = Result<bytes::Bytes, std::io::Error>> + Unpin>>;

struct FetchResponseBodyResource {
  reader: AsyncRefCell<StreamReader<BytesStream, bytes::Bytes>>,
  cancel: CancelHandle,
}

impl Resource for FetchResponseBodyResource {
  fn name(&self) -> Cow<str> {
    "fetchResponseBody".into()
  }
}

struct HttpClientResource {
  client: Client,
}

impl Resource for HttpClientResource {
  fn name(&self) -> Cow<str> {
    "httpClient".into()
  }
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
    let permissions = state.borrow_mut::<FP>();
    permissions.check_read(&PathBuf::from(ca_file))?;
  }

  let client =
    create_http_client(args.ca_file.as_deref(), args.ca_data.as_deref())
      .unwrap();

  let rid = state.resource_table.add(HttpClientResource::new(client));
  Ok(json!(rid))
}

/// Create new instance of async reqwest::Client. This client supports
/// proxies and doesn't follow redirects.
fn create_http_client(
  ca_file: Option<&str>,
  ca_data: Option<&str>,
) -> Result<Client, AnyError> {
  let mut builder = Client::builder().redirect(Policy::none()).use_rustls_tls();
  if let Some(ca_data) = ca_data {
    let ca_data_vec = ca_data.as_bytes().to_vec();
    let cert = reqwest::Certificate::from_pem(&ca_data_vec)?;
    builder = builder.add_root_certificate(cert);
  } else if let Some(ca_file) = ca_file {
    let mut buf = Vec::new();
    File::open(ca_file)?.read_to_end(&mut buf)?;
    let cert = reqwest::Certificate::from_pem(&buf)?;
    builder = builder.add_root_certificate(cert);
  }
  builder
    .build()
    .map_err(|_| deno_core::error::generic_error("Unable to build http client"))
}
