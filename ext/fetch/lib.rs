// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use data_url::DataUrl;
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
use deno_core::ByteString;
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
use deno_tls::create_http_client;
use deno_tls::rustls::RootCertStore;
use deno_tls::Proxy;
use http::header::CONTENT_LENGTH;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::header::HOST;
use reqwest::Body;
use reqwest::Client;
use reqwest::Method;
use reqwest::RequestBuilder;
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

// Re-export reqwest and data_url
pub use data_url;
pub use reqwest;

pub fn init<P: FetchPermissions + 'static>(
  user_agent: String,
  root_cert_store: Option<RootCertStore>,
  proxy: Option<Proxy>,
  request_builder_hook: Option<fn(RequestBuilder) -> RequestBuilder>,
  unsafely_ignore_certificate_errors: Option<Vec<String>>,
  client_cert_chain_and_key: Option<(String, String)>,
) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/fetch",
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
        create_http_client(
          user_agent.clone(),
          root_cert_store.clone(),
          None,
          proxy.clone(),
          unsafely_ignore_certificate_errors.clone(),
          client_cert_chain_and_key.clone(),
        )
        .unwrap()
      });
      state.put::<HttpClientDefaults>(HttpClientDefaults {
        user_agent: user_agent.clone(),
        root_cert_store: root_cert_store.clone(),
        proxy: proxy.clone(),
        request_builder_hook,
        unsafely_ignore_certificate_errors: unsafely_ignore_certificate_errors
          .clone(),
        client_cert_chain_and_key: client_cert_chain_and_key.clone(),
      });
      Ok(())
    })
    .build()
}

pub struct HttpClientDefaults {
  pub user_agent: String,
  pub root_cert_store: Option<RootCertStore>,
  pub proxy: Option<Proxy>,
  pub request_builder_hook: Option<fn(RequestBuilder) -> RequestBuilder>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub client_cert_chain_and_key: Option<(String, String)>,
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
  method: ByteString,
  url: String,
  headers: Vec<(ByteString, ByteString)>,
  client_rid: Option<u32>,
  has_body: bool,
  body_length: Option<u64>,
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
    let r = state.resource_table.get::<HttpClientResource>(rid)?;
    r.client.clone()
  } else {
    let client = state.borrow::<reqwest::Client>();
    client.clone()
  };

  let method = Method::from_bytes(&args.method)?;
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

            // If the size of the body is known, we include a content-length
            // header explicitly.
            if let Some(body_size) = args.body_length {
              request =
                request.header(CONTENT_LENGTH, HeaderValue::from(body_size))
            }

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
        let name = HeaderName::from_bytes(&key)
          .map_err(|err| type_error(err.to_string()))?;
        let v = HeaderValue::from_bytes(&value)
          .map_err(|err| type_error(err.to_string()))?;
        if name != HOST {
          request = request.header(name, v);
        }
      }

      let defaults = state.borrow::<HttpClientDefaults>();
      if let Some(request_builder_hook) = defaults.request_builder_hook {
        request = request_builder_hook(request);
      }

      let cancel_handle = CancelHandle::new_rc();
      let cancel_handle_ = cancel_handle.clone();

      let fut = async move {
        request
          .send()
          .or_cancel(cancel_handle_)
          .await
          .map(|res| res.map_err(|err| type_error(err.to_string())))
      };

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
      // Blob URL resolution happens in the JS side of fetch. If we got here is
      // because the URL isn't an object URL.
      return Err(type_error("Blob for the given URL not found."));
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
  headers: Vec<(ByteString, ByteString)>,
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
    .take::<FetchRequestResource>(rid)?;

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
    let key_bytes: &[u8] = key.as_ref();
    res_headers.push((
      ByteString(key_bytes.to_owned()),
      ByteString(val.as_bytes().to_owned()),
    ));
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
    .get::<FetchRequestBodyResource>(rid)?;
  let body = RcRef::map(&resource, |r| &r.body).borrow_mut().await;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  body.send(Ok(buf)).or_cancel(cancel).await?.map_err(|_| {
    type_error("request body receiver not connected (request closed)")
  })?;

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
    .get::<FetchResponseBodyResource>(rid)?;
  let mut reader = RcRef::map(&resource, |r| &r.reader).borrow_mut().await;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let mut buf = data.clone();
  let read = reader.read(&mut buf).try_or_cancel(cancel).await?;
  Ok(read)
}

type CancelableResponseResult = Result<Result<Response, AnyError>, Canceled>;

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
  ca_stores: Option<Vec<String>>,
  ca_file: Option<String>,
  ca_data: Option<ByteString>,
  proxy: Option<Proxy>,
  cert_chain: Option<String>,
  private_key: Option<String>,
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

  let client_cert_chain_and_key = {
    if args.cert_chain.is_some() || args.private_key.is_some() {
      let cert_chain = args
        .cert_chain
        .ok_or_else(|| type_error("No certificate chain provided"))?;
      let private_key = args
        .private_key
        .ok_or_else(|| type_error("No private key provided"))?;

      Some((cert_chain, private_key))
    } else {
      None
    }
  };

  let defaults = state.borrow::<HttpClientDefaults>();
  let cert_data =
    get_cert_data(args.ca_file.as_deref(), args.ca_data.as_deref())?;

  let client = create_http_client(
    defaults.user_agent.clone(),
    defaults.root_cert_store.clone(),
    cert_data,
    args.proxy,
    defaults.unsafely_ignore_certificate_errors.clone(),
    client_cert_chain_and_key,
  )?;

  let rid = state.resource_table.add(HttpClientResource::new(client));
  Ok(rid)
}

fn get_cert_data(
  ca_file: Option<&str>,
  ca_data: Option<&[u8]>,
) -> Result<Option<Vec<u8>>, AnyError> {
  if let Some(ca_data) = ca_data {
    Ok(Some(ca_data.to_vec()))
  } else if let Some(ca_file) = ca_file {
    let mut buf = Vec::new();
    File::open(ca_file)?.read_to_end(&mut buf)?;
    Ok(Some(buf))
  } else {
    Ok(None)
  }
}
