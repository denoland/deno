// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::generic_error;
use deno_core::error::null_opbuf;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::Future;
use deno_core::futures::Stream;
use deno_core::futures::StreamExt;
use deno_core::include_js_files;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::url::Url;
use deno_core::AsyncRefCell;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::Canceled;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;

use data_url::DataUrl;
use deno_web::BlobUrlStore;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::header::HOST;
use reqwest::header::USER_AGENT;
use reqwest::redirect::Policy;
use reqwest::Body;
use reqwest::Client;
use reqwest::Method;
use reqwest::Response;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::From;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::io::StreamReader;

pub use reqwest; // Re-export reqwest

pub fn init<P: FetchPermissions + 'static>(
  user_agent: String,
  ca_data: Option<Vec<u8>>,
  proxy: Option<Proxy>,
) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:extensions/fetch",
      "01_fetch_util.js",
      "20_headers.js",
      "21_formdata.js",
      "22_body.js",
      "22_http_client.js",
      "23_request.js",
      "23_response.js",
      "26_fetch.js",
    ))
    .ops(vec![
      ("op_fetch", op_sync(op_fetch::<P>)),
      ("op_fetch_send", op_async(op_fetch_send)),
      ("op_fetch_request_write", op_async(op_fetch_request_write)),
      ("op_fetch_response_read", op_async(op_fetch_response_read)),
      ("op_create_http_client", op_sync(op_create_http_client::<P>)),
    ])
    .state(move |state| {
      state.put::<reqwest::Client>({
        create_http_client(user_agent.clone(), ca_data.clone(), proxy.clone())
          .unwrap()
      });
      state.put::<HttpClientDefaults>(HttpClientDefaults {
        ca_data: ca_data.clone(),
        user_agent: user_agent.clone(),
        proxy: proxy.clone(),
      });
      Ok(())
    })
    .build()
}

pub struct HttpClientDefaults {
  pub user_agent: String,
  pub ca_data: Option<Vec<u8>>,
  pub proxy: Option<Proxy>,
}

pub trait FetchPermissions {
  fn check_net_url(&mut self, _url: &Url) -> Result<(), AnyError>;
  fn check_read(&mut self, _p: &Path) -> Result<(), AnyError>;
}

/// For use with `op_fetch` when the user does not want permissions.
pub struct NoFetchPermissions;

impl FetchPermissions for NoFetchPermissions {
  fn check_net_url(&mut self, _url: &Url) -> Result<(), AnyError> {
    Ok(())
  }

  fn check_read(&mut self, _p: &Path) -> Result<(), AnyError> {
    Ok(())
  }
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_fetch.d.ts")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchArgs {
  method: String,
  url: String,
  headers: Vec<(String, String)>,
  client_rid: Option<u32>,
  has_body: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchReturn {
  request_rid: ResourceId,
  request_body_rid: Option<ResourceId>,
  cancel_handle_rid: Option<ResourceId>,
}

pub fn op_fetch<FP>(
  state: &mut OpState,
  args: FetchArgs,
  data: Option<ZeroCopyBuf>,
) -> Result<FetchReturn, AnyError>
where
  FP: FetchPermissions + 'static,
{
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

  let method = Method::from_bytes(args.method.as_bytes())?;
  let url = Url::parse(&args.url)?;

  // Check scheme before asking for net permission
  let scheme = url.scheme();
  let (request_rid, request_body_rid, cancel_handle_rid) = match scheme {
    "http" | "https" => {
      let permissions = state.borrow_mut::<FP>();
      permissions.check_net_url(&url)?;

      let mut request = client.request(method, url);

      let request_body_rid = if args.has_body {
        match data {
          None => {
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
          Some(data) => {
            // If a body is passed, we use it, and don't return a body for streaming.
            request = request.body(Vec::from(&*data));
            None
          }
        }
      } else {
        None
      };

      for (key, value) in args.headers {
        let name = HeaderName::from_bytes(key.as_bytes()).unwrap();
        let v = HeaderValue::from_str(&value).unwrap();
        if name != HOST {
          request = request.header(name, v);
        }
      }

      let cancel_handle = CancelHandle::new_rc();
      let cancel_handle_ = cancel_handle.clone();

      let fut = async move { request.send().or_cancel(cancel_handle_).await };

      let request_rid = state
        .resource_table
        .add(FetchRequestResource(Box::pin(fut)));

      let cancel_handle_rid =
        state.resource_table.add(FetchCancelHandle(cancel_handle));

      (request_rid, request_body_rid, Some(cancel_handle_rid))
    }
    "data" => {
      let data_url = DataUrl::process(url.as_str())
        .map_err(|e| type_error(format!("{:?}", e)))?;

      let (body, _) = data_url
        .decode_to_vec()
        .map_err(|e| type_error(format!("{:?}", e)))?;

      let response = http::Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, data_url.mime_type().to_string())
        .body(reqwest::Body::from(body))?;

      let fut = async move { Ok(Ok(Response::from(response))) };

      let request_rid = state
        .resource_table
        .add(FetchRequestResource(Box::pin(fut)));

      (request_rid, None, None)
    }
    "blob" => {
      let blob_url_storage =
        state.try_borrow::<BlobUrlStore>().ok_or_else(|| {
          type_error("Blob URLs are not supported in this context.")
        })?;

      let blob = blob_url_storage
        .get(url)?
        .ok_or_else(|| type_error("Blob for the given URL not found."))?;

      if method != "GET" {
        return Err(type_error("Blob URL fetch only supports GET method."));
      }

      let response = http::Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_LENGTH, blob.data.len())
        .header(http::header::CONTENT_TYPE, blob.media_type)
        .body(reqwest::Body::from(blob.data))?;

      let fut = async move { Ok(Ok(Response::from(response))) };

      let request_rid = state
        .resource_table
        .add(FetchRequestResource(Box::pin(fut)));

      (request_rid, None, None)
    }
    _ => return Err(type_error(format!("scheme '{}' not supported", scheme))),
  };

  Ok(FetchReturn {
    request_rid,
    request_body_rid,
    cancel_handle_rid,
  })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResponse {
  status: u16,
  status_text: String,
  headers: Vec<(String, String)>,
  url: String,
  response_rid: ResourceId,
}

pub async fn op_fetch_send(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _: (),
) -> Result<FetchResponse, AnyError> {
  let request = state
    .borrow_mut()
    .resource_table
    .take::<FetchRequestResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let request = Rc::try_unwrap(request)
    .ok()
    .expect("multiple op_fetch_send ongoing");

  let res = match request.0.await {
    Ok(Ok(res)) => res,
    Ok(Err(err)) => return Err(type_error(err.to_string())),
    Err(_) => return Err(type_error("request was cancelled")),
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

  Ok(FetchResponse {
    status: status.as_u16(),
    status_text: status.canonical_reason().unwrap_or("").to_string(),
    headers: res_headers,
    url,
    response_rid: rid,
  })
}

pub async fn op_fetch_request_write(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  data: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let data = data.ok_or_else(null_opbuf)?;
  let buf = Vec::from(&*data);

  let resource = state
    .borrow()
    .resource_table
    .get::<FetchRequestBodyResource>(rid)
    .ok_or_else(bad_resource_id)?;
  let body = RcRef::map(&resource, |r| &r.body).borrow_mut().await;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  body.send(Ok(buf)).or_cancel(cancel).await??;

  Ok(())
}

pub async fn op_fetch_response_read(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  data: Option<ZeroCopyBuf>,
) -> Result<usize, AnyError> {
  let data = data.ok_or_else(null_opbuf)?;

  let resource = state
    .borrow()
    .resource_table
    .get::<FetchResponseBodyResource>(rid)
    .ok_or_else(bad_resource_id)?;
  let mut reader = RcRef::map(&resource, |r| &r.reader).borrow_mut().await;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let mut buf = data.clone();
  let read = reader.read(&mut buf).try_or_cancel(cancel).await?;
  Ok(read)
}

type CancelableResponseResult =
  Result<Result<Response, reqwest::Error>, Canceled>;

struct FetchRequestResource(
  Pin<Box<dyn Future<Output = CancelableResponseResult>>>,
);

impl Resource for FetchRequestResource {
  fn name(&self) -> Cow<str> {
    "fetchRequest".into()
  }
}

struct FetchCancelHandle(Rc<CancelHandle>);

impl Resource for FetchCancelHandle {
  fn name(&self) -> Cow<str> {
    "fetchCancelHandle".into()
  }

  fn close(self: Rc<Self>) {
    self.0.cancel()
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

  fn close(self: Rc<Self>) {
    self.cancel.cancel()
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

  fn close(self: Rc<Self>) {
    self.cancel.cancel()
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

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct CreateHttpClientOptions {
  ca_file: Option<String>,
  ca_data: Option<String>,
  proxy: Option<Proxy>,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct Proxy {
  pub url: String,
  pub basic_auth: Option<BasicAuth>,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct BasicAuth {
  pub username: String,
  pub password: String,
}

pub fn op_create_http_client<FP>(
  state: &mut OpState,
  args: CreateHttpClientOptions,
  _: (),
) -> Result<ResourceId, AnyError>
where
  FP: FetchPermissions + 'static,
{
  if let Some(ca_file) = args.ca_file.clone() {
    let permissions = state.borrow_mut::<FP>();
    permissions.check_read(&PathBuf::from(ca_file))?;
  }

  if let Some(proxy) = args.proxy.clone() {
    let permissions = state.borrow_mut::<FP>();
    let url = Url::parse(&proxy.url)?;
    permissions.check_net_url(&url)?;
  }

  let defaults = state.borrow::<HttpClientDefaults>();

  let cert_data =
    get_cert_data(args.ca_file.as_deref(), args.ca_data.as_deref())?;
  let client = create_http_client(
    defaults.user_agent.clone(),
    cert_data.or_else(|| defaults.ca_data.clone()),
    args.proxy,
  )
  .unwrap();

  let rid = state.resource_table.add(HttpClientResource::new(client));
  Ok(rid)
}

fn get_cert_data(
  ca_file: Option<&str>,
  ca_data: Option<&str>,
) -> Result<Option<Vec<u8>>, AnyError> {
  if let Some(ca_data) = ca_data {
    Ok(Some(ca_data.as_bytes().to_vec()))
  } else if let Some(ca_file) = ca_file {
    let mut buf = Vec::new();
    File::open(ca_file)?.read_to_end(&mut buf)?;
    Ok(Some(buf))
  } else {
    Ok(None)
  }
}

/// Create new instance of async reqwest::Client. This client supports
/// proxies and doesn't follow redirects.
pub fn create_http_client(
  user_agent: String,
  ca_data: Option<Vec<u8>>,
  proxy: Option<Proxy>,
) -> Result<Client, AnyError> {
  let mut headers = HeaderMap::new();
  headers.insert(USER_AGENT, user_agent.parse().unwrap());
  let mut builder = Client::builder()
    .redirect(Policy::none())
    .default_headers(headers)
    .use_rustls_tls();

  if let Some(ca_data) = ca_data {
    let cert = reqwest::Certificate::from_pem(&ca_data)?;
    builder = builder.add_root_certificate(cert);
  }

  if let Some(proxy) = proxy {
    let mut reqwest_proxy = reqwest::Proxy::all(&proxy.url)?;
    if let Some(basic_auth) = &proxy.basic_auth {
      reqwest_proxy =
        reqwest_proxy.basic_auth(&basic_auth.username, &basic_auth.password);
    }
    builder = builder.proxy(reqwest_proxy);
  }

  builder
    .build()
    .map_err(|e| generic_error(format!("Unable to build http client: {}", e)))
}
