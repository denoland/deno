// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod fs_fetch_handler;

use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::min;
use std::convert::From;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use bytes::Bytes;
use deno_core::anyhow::Error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::stream::Peekable;
use deno_core::futures::Future;
use deno_core::futures::FutureExt;
use deno_core::futures::Stream;
use deno_core::futures::StreamExt;
use deno_core::op2;
use deno_core::unsync::spawn;
use deno_core::url::Url;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::Canceled;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_tls::rustls::RootCertStore;
use deno_tls::Proxy;
use deno_tls::RootCertStoreProvider;

use data_url::DataUrl;
use deno_tls::TlsKey;
use deno_tls::TlsKeys;
use deno_tls::TlsKeysHolder;
use http_v02::header::CONTENT_LENGTH;
use http_v02::Uri;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::header::ACCEPT_ENCODING;
use reqwest::header::HOST;
use reqwest::header::RANGE;
use reqwest::header::USER_AGENT;
use reqwest::redirect::Policy;
use reqwest::Body;
use reqwest::Client;
use reqwest::Method;
use reqwest::RequestBuilder;
use reqwest::Response;
use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

// Re-export reqwest and data_url
pub use data_url;
pub use reqwest;

pub use fs_fetch_handler::FsFetchHandler;

#[derive(Clone)]
pub struct Options {
  pub user_agent: String,
  pub root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
  pub proxy: Option<Proxy>,
  pub request_builder_hook:
    Option<fn(RequestBuilder) -> Result<RequestBuilder, AnyError>>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub client_cert_chain_and_key: TlsKeys,
  pub file_fetch_handler: Rc<dyn FetchHandler>,
}

impl Options {
  pub fn root_cert_store(&self) -> Result<Option<RootCertStore>, AnyError> {
    Ok(match &self.root_cert_store_provider {
      Some(provider) => Some(provider.get_or_try_init()?.clone()),
      None => None,
    })
  }
}

impl Default for Options {
  fn default() -> Self {
    Self {
      user_agent: "".to_string(),
      root_cert_store_provider: None,
      proxy: None,
      request_builder_hook: None,
      unsafely_ignore_certificate_errors: None,
      client_cert_chain_and_key: TlsKeys::Null,
      file_fetch_handler: Rc::new(DefaultFileFetchHandler),
    }
  }
}

deno_core::extension!(deno_fetch,
  deps = [ deno_webidl, deno_web, deno_url, deno_console ],
  parameters = [FP: FetchPermissions],
  ops = [
    op_fetch<FP>,
    op_fetch_send,
    op_fetch_response_upgrade,
    op_utf8_to_byte_string,
    op_fetch_custom_client<FP>,
  ],
  esm = [
    "20_headers.js",
    "21_formdata.js",
    "22_body.js",
    "22_http_client.js",
    "23_request.js",
    "23_response.js",
    "26_fetch.js",
    "27_eventsource.js"
  ],
  options = {
    options: Options,
  },
  state = |state, options| {
    state.put::<Options>(options.options);
  },
);

pub type CancelableResponseFuture =
  Pin<Box<dyn Future<Output = CancelableResponseResult>>>;

pub trait FetchHandler: dyn_clone::DynClone {
  // Return the result of the fetch request consisting of a tuple of the
  // cancelable response result, the optional fetch body resource and the
  // optional cancel handle.
  fn fetch_file(
    &self,
    state: &mut OpState,
    url: Url,
  ) -> (CancelableResponseFuture, Option<Rc<CancelHandle>>);
}

dyn_clone::clone_trait_object!(FetchHandler);

/// A default implementation which will error for every request.
#[derive(Clone)]
pub struct DefaultFileFetchHandler;

impl FetchHandler for DefaultFileFetchHandler {
  fn fetch_file(
    &self,
    _state: &mut OpState,
    _url: Url,
  ) -> (CancelableResponseFuture, Option<Rc<CancelHandle>>) {
    let fut = async move {
      Ok(Err(type_error(
        "NetworkError when attempting to fetch resource.",
      )))
    };
    (Box::pin(fut), None)
  }
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_fetch.d.ts")
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchReturn {
  pub request_rid: ResourceId,
  pub cancel_handle_rid: Option<ResourceId>,
}

pub fn get_or_create_client_from_state(
  state: &mut OpState,
) -> Result<reqwest::Client, AnyError> {
  if let Some(client) = state.try_borrow::<reqwest::Client>() {
    Ok(client.clone())
  } else {
    let options = state.borrow::<Options>();
    let client = create_client_from_options(options)?;
    state.put::<reqwest::Client>(client.clone());
    Ok(client)
  }
}

pub fn create_client_from_options(
  options: &Options,
) -> Result<reqwest::Client, AnyError> {
  create_http_client(
    &options.user_agent,
    CreateHttpClientOptions {
      root_cert_store: options.root_cert_store()?,
      ca_certs: vec![],
      proxy: options.proxy.clone(),
      unsafely_ignore_certificate_errors: options
        .unsafely_ignore_certificate_errors
        .clone(),
      client_cert_chain_and_key: options
        .client_cert_chain_and_key
        .clone()
        .try_into()
        .unwrap_or_default(),
      pool_max_idle_per_host: None,
      pool_idle_timeout: None,
      http1: true,
      http2: true,
    },
  )
}

#[allow(clippy::type_complexity)]
pub struct ResourceToBodyAdapter(
  Rc<dyn Resource>,
  Option<Pin<Box<dyn Future<Output = Result<BufView, Error>>>>>,
);

impl ResourceToBodyAdapter {
  pub fn new(resource: Rc<dyn Resource>) -> Self {
    let future = resource.clone().read(64 * 1024);
    Self(resource, Some(future))
  }
}

// SAFETY: we only use this on a single-threaded executor
unsafe impl Send for ResourceToBodyAdapter {}
// SAFETY: we only use this on a single-threaded executor
unsafe impl Sync for ResourceToBodyAdapter {}

impl Stream for ResourceToBodyAdapter {
  type Item = Result<Bytes, Error>;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Self::Item>> {
    let this = self.get_mut();
    if let Some(mut fut) = this.1.take() {
      match fut.poll_unpin(cx) {
        Poll::Pending => {
          this.1 = Some(fut);
          Poll::Pending
        }
        Poll::Ready(res) => match res {
          Ok(buf) if buf.is_empty() => Poll::Ready(None),
          Ok(_) => {
            this.1 = Some(this.0.clone().read(64 * 1024));
            Poll::Ready(Some(res.map(|b| b.to_vec().into())))
          }
          _ => Poll::Ready(Some(res.map(|b| b.to_vec().into()))),
        },
      }
    } else {
      Poll::Ready(None)
    }
  }
}

impl Drop for ResourceToBodyAdapter {
  fn drop(&mut self) {
    self.0.clone().close()
  }
}

pub trait FetchPermissions {
  fn check_net_url(
    &mut self,
    _url: &Url,
    api_name: &str,
  ) -> Result<(), AnyError>;
  fn check_read(&mut self, _p: &Path, api_name: &str) -> Result<(), AnyError>;
}

impl FetchPermissions for deno_permissions::PermissionsContainer {
  #[inline(always)]
  fn check_net_url(
    &mut self,
    url: &Url,
    api_name: &str,
  ) -> Result<(), AnyError> {
    deno_permissions::PermissionsContainer::check_net_url(self, url, api_name)
  }

  #[inline(always)]
  fn check_read(
    &mut self,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    deno_permissions::PermissionsContainer::check_read(self, path, api_name)
  }
}

#[op2]
#[serde]
#[allow(clippy::too_many_arguments)]
pub fn op_fetch<FP>(
  state: &mut OpState,
  #[serde] method: ByteString,
  #[string] url: String,
  #[serde] headers: Vec<(ByteString, ByteString)>,
  #[smi] client_rid: Option<u32>,
  has_body: bool,
  #[buffer] data: Option<JsBuffer>,
  #[smi] resource: Option<ResourceId>,
) -> Result<FetchReturn, AnyError>
where
  FP: FetchPermissions + 'static,
{
  let (client, allow_host) = if let Some(rid) = client_rid {
    let r = state.resource_table.get::<HttpClientResource>(rid)?;
    (r.client.clone(), r.allow_host)
  } else {
    (get_or_create_client_from_state(state)?, false)
  };

  let method = Method::from_bytes(&method)?;
  let url = Url::parse(&url)?;

  // Check scheme before asking for net permission
  let scheme = url.scheme();
  let (request_rid, cancel_handle_rid) = match scheme {
    "file" => {
      let path = url.to_file_path().map_err(|_| {
        type_error("NetworkError when attempting to fetch resource.")
      })?;
      let permissions = state.borrow_mut::<FP>();
      permissions.check_read(&path, "fetch()")?;

      if method != Method::GET {
        return Err(type_error(format!(
          "Fetching files only supports the GET method. Received {method}."
        )));
      }

      let Options {
        file_fetch_handler, ..
      } = state.borrow_mut::<Options>();
      let file_fetch_handler = file_fetch_handler.clone();
      let (request, maybe_cancel_handle) =
        file_fetch_handler.fetch_file(state, url);
      let request_rid = state.resource_table.add(FetchRequestResource(request));
      let maybe_cancel_handle_rid = maybe_cancel_handle
        .map(|ch| state.resource_table.add(FetchCancelHandle(ch)));

      (request_rid, maybe_cancel_handle_rid)
    }
    "http" | "https" => {
      let permissions = state.borrow_mut::<FP>();
      permissions.check_net_url(&url, "fetch()")?;

      // Make sure that we have a valid URI early, as reqwest's `RequestBuilder::send`
      // internally uses `expect_uri`, which panics instead of returning a usable `Result`.
      if url.as_str().parse::<Uri>().is_err() {
        return Err(type_error("Invalid URL"));
      }

      let mut request = client.request(method.clone(), url);

      if has_body {
        match (data, resource) {
          (Some(data), _) => {
            // If a body is passed, we use it, and don't return a body for streaming.
            request = request.body(data.to_vec());
          }
          (_, Some(resource)) => {
            let resource = state.resource_table.take_any(resource)?;
            match resource.size_hint() {
              (body_size, Some(n)) if body_size == n && body_size > 0 => {
                request =
                  request.header(CONTENT_LENGTH, HeaderValue::from(body_size));
              }
              _ => {}
            }
            request = request
              .body(Body::wrap_stream(ResourceToBodyAdapter::new(resource)))
          }
          (None, None) => unreachable!(),
        }
      } else {
        // POST and PUT requests should always have a 0 length content-length,
        // if there is no body. https://fetch.spec.whatwg.org/#http-network-or-cache-fetch
        if matches!(method, Method::POST | Method::PUT) {
          request = request.header(CONTENT_LENGTH, HeaderValue::from(0));
        }
      };

      let mut header_map = HeaderMap::new();
      for (key, value) in headers {
        let name = HeaderName::from_bytes(&key)
          .map_err(|err| type_error(err.to_string()))?;
        let v = HeaderValue::from_bytes(&value)
          .map_err(|err| type_error(err.to_string()))?;

        if (name != HOST || allow_host) && name != CONTENT_LENGTH {
          header_map.append(name, v);
        }
      }

      if header_map.contains_key(RANGE) {
        // https://fetch.spec.whatwg.org/#http-network-or-cache-fetch step 18
        // If httpRequest’s header list contains `Range`, then append (`Accept-Encoding`, `identity`)
        header_map
          .insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
      }
      request = request.headers(header_map);

      let options = state.borrow::<Options>();
      if let Some(request_builder_hook) = options.request_builder_hook {
        request = request_builder_hook(request)
          .map_err(|err| type_error(err.to_string()))?;
      }

      let cancel_handle = CancelHandle::new_rc();
      let cancel_handle_ = cancel_handle.clone();

      let fut = async move {
        request
          .send()
          .or_cancel(cancel_handle_)
          .await
          .map(|res| res.map_err(|err| err.into()))
      };

      let request_rid = state
        .resource_table
        .add(FetchRequestResource(Box::pin(fut)));

      let cancel_handle_rid =
        state.resource_table.add(FetchCancelHandle(cancel_handle));

      (request_rid, Some(cancel_handle_rid))
    }
    "data" => {
      let data_url = DataUrl::process(url.as_str())
        .map_err(|e| type_error(format!("{e:?}")))?;

      let (body, _) = data_url
        .decode_to_vec()
        .map_err(|e| type_error(format!("{e:?}")))?;

      let response = http_v02::Response::builder()
        .status(http_v02::StatusCode::OK)
        .header(
          http_v02::header::CONTENT_TYPE,
          data_url.mime_type().to_string(),
        )
        .body(reqwest::Body::from(body))?;

      let fut = async move { Ok(Ok(Response::from(response))) };

      let request_rid = state
        .resource_table
        .add(FetchRequestResource(Box::pin(fut)));

      (request_rid, None)
    }
    "blob" => {
      // Blob URL resolution happens in the JS side of fetch. If we got here is
      // because the URL isn't an object URL.
      return Err(type_error("Blob for the given URL not found."));
    }
    _ => return Err(type_error(format!("scheme '{scheme}' not supported"))),
  };

  Ok(FetchReturn {
    request_rid,
    cancel_handle_rid,
  })
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResponse {
  pub status: u16,
  pub status_text: String,
  pub headers: Vec<(ByteString, ByteString)>,
  pub url: String,
  pub response_rid: ResourceId,
  pub content_length: Option<u64>,
  pub remote_addr_ip: Option<String>,
  pub remote_addr_port: Option<u16>,
  pub error: Option<String>,
}

#[op2(async)]
#[serde]
pub async fn op_fetch_send(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
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
    Ok(Err(err)) => {
      // We're going to try and rescue the error cause from a stream and return it from this fetch.
      // If any error in the chain is a reqwest body error, return that as a special result we can use to
      // reconstruct an error chain (eg: `new TypeError(..., { cause: new Error(...) })`).
      // TODO(mmastrac): it would be a lot easier if we just passed a v8::Global through here instead
      let mut err_ref: &dyn std::error::Error = err.as_ref();
      while let Some(err) = std::error::Error::source(err_ref) {
        if let Some(err) = err.downcast_ref::<reqwest::Error>() {
          if err.is_body() {
            // Extracts the next error cause and uses that for the message
            if let Some(err) = std::error::Error::source(err) {
              return Ok(FetchResponse {
                error: Some(err.to_string()),
                ..Default::default()
              });
            }
          }
        }
        err_ref = err;
      }

      return Err(type_error(err.to_string()));
    }
    Err(_) => return Err(type_error("request was cancelled")),
  };

  let status = res.status();
  let url = res.url().to_string();
  let mut res_headers = Vec::new();
  for (key, val) in res.headers().iter() {
    res_headers.push((key.as_str().into(), val.as_bytes().into()));
  }

  let content_length = res.content_length();
  let remote_addr = res.remote_addr();
  let (remote_addr_ip, remote_addr_port) = if let Some(addr) = remote_addr {
    (Some(addr.ip().to_string()), Some(addr.port()))
  } else {
    (None, None)
  };

  let response_rid = state
    .borrow_mut()
    .resource_table
    .add(FetchResponseResource::new(res, content_length));

  Ok(FetchResponse {
    status: status.as_u16(),
    status_text: status.canonical_reason().unwrap_or("").to_string(),
    headers: res_headers,
    url,
    response_rid,
    content_length,
    remote_addr_ip,
    remote_addr_port,
    error: None,
  })
}

#[op2(async)]
#[smi]
pub async fn op_fetch_response_upgrade(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<ResourceId, AnyError> {
  let raw_response = state
    .borrow_mut()
    .resource_table
    .take::<FetchResponseResource>(rid)?;
  let raw_response = Rc::try_unwrap(raw_response)
    .expect("Someone is holding onto FetchResponseResource");

  let (read, write) = tokio::io::duplex(1024);
  let (read_rx, write_tx) = tokio::io::split(read);
  let (mut write_rx, mut read_tx) = tokio::io::split(write);
  let upgraded = raw_response.upgrade().await?;
  {
    // Stage 3: Pump the data
    let (mut upgraded_rx, mut upgraded_tx) = tokio::io::split(upgraded);

    spawn(async move {
      let mut buf = [0; 1024];
      loop {
        let read = upgraded_rx.read(&mut buf).await?;
        if read == 0 {
          break;
        }
        read_tx.write_all(&buf[..read]).await?;
      }
      Ok::<_, AnyError>(())
    });
    spawn(async move {
      let mut buf = [0; 1024];
      loop {
        let read = write_rx.read(&mut buf).await?;
        if read == 0 {
          break;
        }
        upgraded_tx.write_all(&buf[..read]).await?;
      }
      Ok::<_, AnyError>(())
    });
  }

  Ok(
    state
      .borrow_mut()
      .resource_table
      .add(UpgradeStream::new(read_rx, write_tx)),
  )
}

struct UpgradeStream {
  read: AsyncRefCell<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
  write: AsyncRefCell<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
  cancel_handle: CancelHandle,
}

impl UpgradeStream {
  pub fn new(
    read: tokio::io::ReadHalf<tokio::io::DuplexStream>,
    write: tokio::io::WriteHalf<tokio::io::DuplexStream>,
  ) -> Self {
    Self {
      read: AsyncRefCell::new(read),
      write: AsyncRefCell::new(write),
      cancel_handle: CancelHandle::new(),
    }
  }

  async fn read(self: Rc<Self>, buf: &mut [u8]) -> Result<usize, AnyError> {
    let cancel_handle = RcRef::map(self.clone(), |this| &this.cancel_handle);
    async {
      let read = RcRef::map(self, |this| &this.read);
      let mut read = read.borrow_mut().await;
      Ok(Pin::new(&mut *read).read(buf).await?)
    }
    .try_or_cancel(cancel_handle)
    .await
  }

  async fn write(self: Rc<Self>, buf: &[u8]) -> Result<usize, AnyError> {
    let cancel_handle = RcRef::map(self.clone(), |this| &this.cancel_handle);
    async {
      let write = RcRef::map(self, |this| &this.write);
      let mut write = write.borrow_mut().await;
      Ok(Pin::new(&mut *write).write(buf).await?)
    }
    .try_or_cancel(cancel_handle)
    .await
  }
}

impl Resource for UpgradeStream {
  fn name(&self) -> Cow<str> {
    "fetchUpgradedStream".into()
  }

  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel();
  }
}

type CancelableResponseResult = Result<Result<Response, AnyError>, Canceled>;

pub struct FetchRequestResource(
  pub Pin<Box<dyn Future<Output = CancelableResponseResult>>>,
);

impl Resource for FetchRequestResource {
  fn name(&self) -> Cow<str> {
    "fetchRequest".into()
  }
}

pub struct FetchCancelHandle(pub Rc<CancelHandle>);

impl Resource for FetchCancelHandle {
  fn name(&self) -> Cow<str> {
    "fetchCancelHandle".into()
  }

  fn close(self: Rc<Self>) {
    self.0.cancel()
  }
}

type BytesStream =
  Pin<Box<dyn Stream<Item = Result<bytes::Bytes, std::io::Error>> + Unpin>>;

pub enum FetchResponseReader {
  Start(Response),
  BodyReader(Peekable<BytesStream>),
}

impl Default for FetchResponseReader {
  fn default() -> Self {
    let stream: BytesStream = Box::pin(deno_core::futures::stream::empty());
    Self::BodyReader(stream.peekable())
  }
}
#[derive(Debug)]
pub struct FetchResponseResource {
  pub response_reader: AsyncRefCell<FetchResponseReader>,
  pub cancel: CancelHandle,
  pub size: Option<u64>,
}

impl FetchResponseResource {
  pub fn new(response: Response, size: Option<u64>) -> Self {
    Self {
      response_reader: AsyncRefCell::new(FetchResponseReader::Start(response)),
      cancel: CancelHandle::default(),
      size,
    }
  }

  pub async fn upgrade(self) -> Result<reqwest::Upgraded, AnyError> {
    let reader = self.response_reader.into_inner();
    match reader {
      FetchResponseReader::Start(resp) => Ok(resp.upgrade().await?),
      _ => unreachable!(),
    }
  }
}

impl Resource for FetchResponseResource {
  fn name(&self) -> Cow<str> {
    "fetchResponse".into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(async move {
      let mut reader =
        RcRef::map(&self, |r| &r.response_reader).borrow_mut().await;

      let body = loop {
        match &mut *reader {
          FetchResponseReader::BodyReader(reader) => break reader,
          FetchResponseReader::Start(_) => {}
        }

        match std::mem::take(&mut *reader) {
          FetchResponseReader::Start(resp) => {
            let stream: BytesStream = Box::pin(resp.bytes_stream().map(|r| {
              r.map_err(|err| {
                std::io::Error::new(std::io::ErrorKind::Other, err)
              })
            }));
            *reader = FetchResponseReader::BodyReader(stream.peekable());
          }
          FetchResponseReader::BodyReader(_) => unreachable!(),
        }
      };
      let fut = async move {
        let mut reader = Pin::new(body);
        loop {
          match reader.as_mut().peek_mut().await {
            Some(Ok(chunk)) if !chunk.is_empty() => {
              let len = min(limit, chunk.len());
              let chunk = chunk.split_to(len);
              break Ok(chunk.into());
            }
            // This unwrap is safe because `peek_mut()` returned `Some`, and thus
            // currently has a peeked value that can be synchronously returned
            // from `next()`.
            //
            // The future returned from `next()` is always ready, so we can
            // safely call `await` on it without creating a race condition.
            Some(_) => match reader.as_mut().next().await.unwrap() {
              Ok(chunk) => assert!(chunk.is_empty()),
              Err(err) => break Err(type_error(err.to_string())),
            },
            None => break Ok(BufView::empty()),
          }
        }
      };

      let cancel_handle = RcRef::map(self, |r| &r.cancel);
      fut.try_or_cancel(cancel_handle).await
    })
  }

  fn size_hint(&self) -> (u64, Option<u64>) {
    (self.size.unwrap_or(0), self.size)
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel()
  }
}

pub struct HttpClientResource {
  pub client: Client,
  pub allow_host: bool,
}

impl Resource for HttpClientResource {
  fn name(&self) -> Cow<str> {
    "httpClient".into()
  }
}

impl HttpClientResource {
  fn new(client: Client, allow_host: bool) -> Self {
    Self { client, allow_host }
  }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateHttpClientArgs {
  ca_certs: Vec<String>,
  proxy: Option<Proxy>,
  pool_max_idle_per_host: Option<usize>,
  pool_idle_timeout: Option<serde_json::Value>,
  #[serde(default = "default_true")]
  http1: bool,
  #[serde(default = "default_true")]
  http2: bool,
  #[serde(default)]
  allow_host: bool,
}

fn default_true() -> bool {
  true
}

#[op2]
#[smi]
pub fn op_fetch_custom_client<FP>(
  state: &mut OpState,
  #[serde] args: CreateHttpClientArgs,
  #[cppgc] tls_keys: &TlsKeysHolder,
) -> Result<ResourceId, AnyError>
where
  FP: FetchPermissions + 'static,
{
  if let Some(proxy) = args.proxy.clone() {
    let permissions = state.borrow_mut::<FP>();
    let url = Url::parse(&proxy.url)?;
    permissions.check_net_url(&url, "Deno.createHttpClient()")?;
  }

  let options = state.borrow::<Options>();
  let ca_certs = args
    .ca_certs
    .into_iter()
    .map(|cert| cert.into_bytes())
    .collect::<Vec<_>>();

  let client = create_http_client(
    &options.user_agent,
    CreateHttpClientOptions {
      root_cert_store: options.root_cert_store()?,
      ca_certs,
      proxy: args.proxy,
      unsafely_ignore_certificate_errors: options
        .unsafely_ignore_certificate_errors
        .clone(),
      client_cert_chain_and_key: tls_keys.take().try_into().unwrap(),
      pool_max_idle_per_host: args.pool_max_idle_per_host,
      pool_idle_timeout: args.pool_idle_timeout.and_then(
        |timeout| match timeout {
          serde_json::Value::Bool(true) => None,
          serde_json::Value::Bool(false) => Some(None),
          serde_json::Value::Number(specify) => {
            Some(Some(specify.as_u64().unwrap_or_default()))
          }
          _ => Some(None),
        },
      ),
      http1: args.http1,
      http2: args.http2,
    },
  )?;

  let rid = state
    .resource_table
    .add(HttpClientResource::new(client, args.allow_host));
  Ok(rid)
}

#[derive(Debug, Clone)]
pub struct CreateHttpClientOptions {
  pub root_cert_store: Option<RootCertStore>,
  pub ca_certs: Vec<Vec<u8>>,
  pub proxy: Option<Proxy>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub client_cert_chain_and_key: Option<TlsKey>,
  pub pool_max_idle_per_host: Option<usize>,
  pub pool_idle_timeout: Option<Option<u64>>,
  pub http1: bool,
  pub http2: bool,
}

impl Default for CreateHttpClientOptions {
  fn default() -> Self {
    CreateHttpClientOptions {
      root_cert_store: None,
      ca_certs: vec![],
      proxy: None,
      unsafely_ignore_certificate_errors: None,
      client_cert_chain_and_key: None,
      pool_max_idle_per_host: None,
      pool_idle_timeout: None,
      http1: true,
      http2: true,
    }
  }
}

/// Create new instance of async reqwest::Client. This client supports
/// proxies and doesn't follow redirects.
pub fn create_http_client(
  user_agent: &str,
  options: CreateHttpClientOptions,
) -> Result<Client, AnyError> {
  let mut tls_config = deno_tls::create_client_config(
    options.root_cert_store,
    options.ca_certs,
    options.unsafely_ignore_certificate_errors,
    options.client_cert_chain_and_key.into(),
    deno_tls::SocketUse::Http,
  )?;

  let mut alpn_protocols = vec![];
  if options.http2 {
    alpn_protocols.push("h2".into());
  }
  if options.http1 {
    alpn_protocols.push("http/1.1".into());
  }
  tls_config.alpn_protocols = alpn_protocols;

  let mut headers = HeaderMap::new();
  headers.insert(USER_AGENT, user_agent.parse().unwrap());
  let mut builder = Client::builder()
    .redirect(Policy::none())
    .default_headers(headers)
    .use_preconfigured_tls(tls_config);

  if let Some(proxy) = options.proxy {
    let mut reqwest_proxy = reqwest::Proxy::all(&proxy.url)?;
    if let Some(basic_auth) = &proxy.basic_auth {
      reqwest_proxy =
        reqwest_proxy.basic_auth(&basic_auth.username, &basic_auth.password);
    }
    builder = builder.proxy(reqwest_proxy);
  }

  if let Some(pool_max_idle_per_host) = options.pool_max_idle_per_host {
    builder = builder.pool_max_idle_per_host(pool_max_idle_per_host);
  }

  if let Some(pool_idle_timeout) = options.pool_idle_timeout {
    builder = builder.pool_idle_timeout(
      pool_idle_timeout.map(std::time::Duration::from_millis),
    );
  }

  match (options.http1, options.http2) {
    (true, false) => builder = builder.http1_only(),
    (false, true) => builder = builder.http2_prior_knowledge(),
    (true, true) => {}
    (false, false) => {
      return Err(type_error("Either `http1` or `http2` needs to be true"))
    }
  }

  builder.build().map_err(|e| e.into())
}

#[op2]
#[serde]
pub fn op_utf8_to_byte_string(
  #[string] input: String,
) -> Result<ByteString, AnyError> {
  Ok(input.into())
}
