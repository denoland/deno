// Copyright 2018-2026 the Deno authors. MIT license.

pub mod dns;
mod fs_fetch_handler;
mod proxy;
#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::cmp::min;
use std::convert::From;
use std::future;
use std::future::Future;
use std::net::IpAddr;
use std::path::Path;
#[cfg(not(windows))]
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use bytes::Bytes;
// Re-export data_url
pub use data_url;
use data_url::DataUrl;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::Canceled;
use deno_core::ExternalOpsTracker;
use deno_core::FromV8;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ToV8;
use deno_core::convert::ByteString;
use deno_core::convert::Uint8Array;
use deno_core::futures::FutureExt;
use deno_core::futures::Stream;
use deno_core::futures::StreamExt;
use deno_core::futures::TryFutureExt;
use deno_core::op2;
use deno_core::url;
use deno_core::url::Url;
use deno_core::v8;
use deno_error::JsErrorBox;
pub use deno_fs::FsError;
use deno_path_util::PathToUrlError;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionCheckError;
use deno_permissions::PermissionsContainer;
use deno_tls::Proxy;
use deno_tls::RootCertStoreProvider;
use deno_tls::SocketUse;
use deno_tls::TlsKey;
use deno_tls::TlsKeys;
use deno_tls::TlsKeysHolder;
use deno_tls::rustls::RootCertStore;
pub use fs_fetch_handler::FsFetchHandler;
use http::Extensions;
use http::HeaderMap;
use http::Method;
use http::Uri;
use http::header::ACCEPT;
use http::header::ACCEPT_ENCODING;
use http::header::AUTHORIZATION;
use http::header::CONTENT_LENGTH;
use http::header::HOST;
use http::header::HeaderName;
use http::header::HeaderValue;
use http::header::PROXY_AUTHORIZATION;
use http::header::RANGE;
use http::header::USER_AGENT;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::body::Frame;
use hyper_util::client::legacy::Builder as HyperClientBuilder;
use hyper_util::client::legacy::connect::Connection;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::connect::HttpInfo;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::rt::TokioTimer;
pub use proxy::basic_auth;
use tower::BoxError;
use tower::Service;
use tower::ServiceExt;
use tower::retry;
use tower_http::decompression::Decompression;

#[derive(Clone)]
pub struct Options {
  pub user_agent: String,
  pub root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
  pub proxy: Option<Proxy>,
  /// A callback to customize HTTP client configuration.
  ///
  /// The settings applied with this hook may be overridden by the options
  /// provided through `Deno.createHttpClient()` API. For instance, if the hook
  /// calls [`hyper_util::client::legacy::Builder::pool_max_idle_per_host`] with
  /// a value of 99, and a user calls `Deno.createHttpClient({ poolMaxIdlePerHost: 42 })`,
  /// the value that will take effect is 42.
  ///
  /// For more info on what can be configured, see [`hyper_util::client::legacy::Builder`].
  pub client_builder_hook: Option<fn(HyperClientBuilder) -> HyperClientBuilder>,
  #[allow(clippy::type_complexity, reason = "TODO: improve")]
  pub request_builder_hook:
    Option<fn(&mut http::Request<ReqBody>) -> Result<(), JsErrorBox>>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub client_cert_chain_and_key: TlsKeys,
  pub file_fetch_handler: Rc<dyn FetchHandler>,
  pub resolver: dns::Resolver,
}

impl Options {
  pub fn root_cert_store(&self) -> Result<Option<RootCertStore>, JsErrorBox> {
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
      client_builder_hook: None,
      request_builder_hook: None,
      unsafely_ignore_certificate_errors: None,
      client_cert_chain_and_key: TlsKeys::Null,
      file_fetch_handler: Rc::new(DefaultFileFetchHandler),
      resolver: dns::Resolver::default(),
    }
  }
}

deno_core::extension!(deno_fetch,
  deps = [ deno_webidl, deno_web ],
  ops = [
    op_fetch,
    op_fetch_send,
    op_fetch_response_closed,
    op_utf8_to_byte_string,
    op_fetch_custom_client,
    op_fetch_promise_is_settled,
  ],
  lazy_loaded_js = [
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

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum FetchError {
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] PermissionCheckError),
  #[class(type)]
  #[error("NetworkError when attempting to fetch resource")]
  NetworkError,
  #[class(type)]
  #[error("Fetching files only supports the GET method: received {0}")]
  FsNotGet(Method),
  #[class(inherit)]
  #[error(transparent)]
  PathToUrl(#[from] PathToUrlError),
  #[class(type)]
  #[error("Invalid URL {0}")]
  InvalidUrl(Url),
  #[class(type)]
  #[error(transparent)]
  InvalidHeaderName(#[from] http::header::InvalidHeaderName),
  #[class(type)]
  #[error(transparent)]
  InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),
  #[class(type)]
  #[error("{0:?}")]
  DataUrl(data_url::DataUrlError),
  #[class(type)]
  #[error("{0:?}")]
  Base64(data_url::forgiving_base64::InvalidBase64),
  #[class(type)]
  #[error("Blob for the given URL not found.")]
  BlobNotFound,
  #[class(type)]
  #[error("Url scheme '{0}' not supported")]
  SchemeNotSupported(String),
  #[class(type)]
  #[error("Request was cancelled")]
  RequestCanceled,
  #[class(generic)]
  #[error(transparent)]
  Http(#[from] http::Error),
  #[class(inherit)]
  #[error(transparent)]
  ClientCreate(#[from] HttpClientCreateError),
  #[class(inherit)]
  #[error(transparent)]
  Url(#[from] url::ParseError),
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePath(#[from] deno_path_util::UrlToFilePathError),
  #[class(type)]
  #[error(transparent)]
  Method(#[from] http::method::InvalidMethod),
  #[class(inherit)]
  #[error(transparent)]
  ClientSend(#[from] ClientSendError),
  #[class(inherit)]
  #[error(transparent)]
  RequestBuilderHook(JsErrorBox),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(generic)]
  #[error(transparent)]
  Dns(hickory_resolver::ResolveError),
  #[class(generic)]
  #[error(transparent)]
  PermissionCheck(PermissionCheckError),
  #[class(inherit)]
  #[error(transparent)]
  Other(JsErrorBox),
}

impl From<deno_fs::FsError> for FetchError {
  fn from(value: deno_fs::FsError) -> Self {
    match value {
      deno_fs::FsError::Io(_)
      | deno_fs::FsError::FileBusy
      | deno_fs::FsError::NotSupported => FetchError::NetworkError,
      deno_fs::FsError::PermissionCheck(err) => {
        FetchError::PermissionCheck(err)
      }
      deno_fs::FsError::JoinError(err) => {
        FetchError::Other(JsErrorBox::from_err(err))
      }
    }
  }
}

pub type CancelableResponseFuture =
  Pin<Box<dyn Future<Output = CancelableResponseResult>>>;

pub trait FetchHandler {
  // Return the result of the fetch request consisting of a tuple of the
  // cancelable response result, the optional fetch body resource and the
  // optional cancel handle.
  fn fetch_file(
    &self,
    state: &mut OpState,
    url: &Url,
  ) -> (CancelableResponseFuture, Option<Rc<CancelHandle>>);
}

/// A default implementation which will error for every request.
#[derive(Clone)]
pub struct DefaultFileFetchHandler;

impl FetchHandler for DefaultFileFetchHandler {
  fn fetch_file(
    &self,
    _state: &mut OpState,
    _url: &Url,
  ) -> (CancelableResponseFuture, Option<Rc<CancelHandle>>) {
    let fut = async move { Ok(Err(FetchError::NetworkError)) };
    (Box::pin(fut), None)
  }
}

#[derive(ToV8)]
pub struct FetchReturn {
  pub request_rid: ResourceId,
  pub cancel_handle_rid: Option<ResourceId>,
}

pub fn get_or_create_client_from_state(
  state: &mut OpState,
) -> Result<Client, HttpClientCreateError> {
  if let Some(client) = state.try_borrow::<Client>() {
    Ok(client.clone())
  } else {
    let permissions = state.borrow::<PermissionsContainer>().clone();
    let options = state.borrow::<Options>();
    let client = create_client_from_options(options, Some(permissions))?;
    state.put::<Client>(client.clone());
    Ok(client)
  }
}

pub fn create_client_from_options(
  options: &Options,
  permissions: Option<PermissionsContainer>,
) -> Result<Client, HttpClientCreateError> {
  let dns_resolver = match permissions {
    Some(p) => options.resolver.clone().with_permissions(p),
    None => options.resolver.clone(),
  };
  create_http_client(
    &options.user_agent,
    CreateHttpClientOptions {
      root_cert_store: options
        .root_cert_store()
        .map_err(HttpClientCreateError::RootCertStore)?,
      ca_certs: vec![],
      proxy: options.proxy.clone(),
      dns_resolver,
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
      local_address: None,
      client_builder_hook: options.client_builder_hook,
    },
  )
}

#[allow(clippy::type_complexity, reason = "TODO: improve")]
pub struct ResourceToBodyAdapter(
  Rc<dyn Resource>,
  Option<Pin<Box<dyn Future<Output = Result<BufView, JsErrorBox>>>>>,
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
  type Item = Result<Bytes, JsErrorBox>;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Self::Item>> {
    let this = self.get_mut();
    match this.1.take() {
      Some(mut fut) => match fut.poll_unpin(cx) {
        Poll::Pending => {
          this.1 = Some(fut);
          Poll::Pending
        }
        Poll::Ready(res) => match res {
          Ok(buf) if buf.is_empty() => Poll::Ready(None),
          Ok(buf) => {
            this.1 = Some(this.0.clone().read(64 * 1024));
            Poll::Ready(Some(Ok(buf.to_vec().into())))
          }
          Err(err) => Poll::Ready(Some(Err(err))),
        },
      },
      _ => Poll::Ready(None),
    }
  }
}

impl hyper::body::Body for ResourceToBodyAdapter {
  type Data = Bytes;
  type Error = JsErrorBox;

  fn poll_frame(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
    match self.poll_next(cx) {
      Poll::Ready(Some(res)) => Poll::Ready(Some(res.map(Frame::data))),
      Poll::Ready(None) => Poll::Ready(None),
      Poll::Pending => Poll::Pending,
    }
  }
}

impl Drop for ResourceToBodyAdapter {
  fn drop(&mut self) {
    self.0.clone().close()
  }
}

#[op2(stack_trace)]
#[allow(clippy::too_many_arguments, reason = "op")]
#[allow(clippy::large_enum_variant, reason = "TODO: investigate")]
#[allow(clippy::result_large_err, reason = "TODO: investigate")]
pub fn op_fetch(
  state: &mut OpState,
  #[scoped] method: ByteString,
  #[string] url: String,
  #[scoped] headers: Vec<(ByteString, ByteString)>,
  #[smi] client_rid: Option<u32>,
  has_body: bool,
  data: Option<Uint8Array>,
  #[smi] resource: Option<ResourceId>,
) -> Result<FetchReturn, FetchError> {
  let (client, allow_host) = if let Some(rid) = client_rid {
    let r = state.resource_table.get::<HttpClientResource>(rid)?;
    (r.client.clone(), r.allow_host)
  } else {
    (get_or_create_client_from_state(state)?, false)
  };

  let method = Method::from_bytes(&method)?;
  let mut url = Url::parse(&url)?;

  // Check scheme before asking for net permission
  let scheme = url.scheme();
  let (request_rid, cancel_handle_rid) = match scheme {
    "file" => {
      if method != Method::GET {
        return Err(FetchError::FsNotGet(method));
      }
      let Options {
        file_fetch_handler, ..
      } = state.borrow_mut::<Options>();
      let file_fetch_handler = file_fetch_handler.clone();
      let (future, maybe_cancel_handle) =
        file_fetch_handler.fetch_file(state, &url);
      let request_rid = state
        .resource_table
        .add(FetchRequestResource { future, url });
      let maybe_cancel_handle_rid = maybe_cancel_handle
        .map(|ch| state.resource_table.add(FetchCancelHandle(ch)));

      (request_rid, maybe_cancel_handle_rid)
    }
    "http" | "https" => {
      let permissions = state.borrow_mut::<PermissionsContainer>();
      permissions.check_net_url(&url, "fetch()")?;

      let maybe_authority = extract_authority(&mut url);
      let uri = url
        .as_str()
        .parse::<Uri>()
        .map_err(|_| FetchError::InvalidUrl(url.clone()))?;

      let mut con_len = None;
      let body = if has_body {
        match (data, resource) {
          (Some(data), _) => {
            // If a body is passed, we use it, and don't return a body for streaming.
            con_len = Some(data.len() as u64);

            ReqBody::full(data.0.into())
          }
          (_, Some(resource)) => {
            let resource = state.resource_table.take_any(resource)?;
            match resource.size_hint() {
              (body_size, Some(n)) if body_size == n && body_size > 0 => {
                con_len = Some(body_size);
              }
              _ => {}
            }
            ReqBody::streaming(ResourceToBodyAdapter::new(resource))
          }
          (None, None) => unreachable!(),
        }
      } else {
        // POST and PUT requests should always have a 0 length content-length,
        // if there is no body. https://fetch.spec.whatwg.org/#http-network-or-cache-fetch
        if matches!(method, Method::POST | Method::PUT) {
          con_len = Some(0);
        }
        ReqBody::empty()
      };

      let mut request = http::Request::new(body);
      *request.method_mut() = method.clone();
      *request.uri_mut() = uri.clone();

      if let Some((username, password)) = maybe_authority {
        request.headers_mut().insert(
          AUTHORIZATION,
          proxy::basic_auth(&username, password.as_deref()),
        );
      }
      if let Some(len) = con_len {
        request.headers_mut().insert(CONTENT_LENGTH, len.into());
      }

      for (key, value) in headers {
        let name = HeaderName::from_bytes(&key)?;
        let v = HeaderValue::from_bytes(&value)?;

        if (name != HOST || allow_host) && name != CONTENT_LENGTH {
          request.headers_mut().append(name, v);
        }
      }

      if request.headers().contains_key(RANGE) {
        // https://fetch.spec.whatwg.org/#http-network-or-cache-fetch step 18
        // If httpRequest’s header list contains `Range`, then append (`Accept-Encoding`, `identity`)
        request
          .headers_mut()
          .insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
      }

      let options = state.borrow::<Options>();
      if let Some(request_builder_hook) = options.request_builder_hook {
        request_builder_hook(&mut request)
          .map_err(FetchError::RequestBuilderHook)?;
      }

      let cancel_handle = CancelHandle::new_rc();
      let cancel_handle_ = cancel_handle.clone();

      let fut = async move {
        client
          .send(request)
          .map_err(Into::into)
          .or_cancel(cancel_handle_)
          .await
      };

      let request_rid = state.resource_table.add(FetchRequestResource {
        future: Box::pin(fut),
        url,
      });

      let cancel_handle_rid =
        state.resource_table.add(FetchCancelHandle(cancel_handle));

      (request_rid, Some(cancel_handle_rid))
    }
    "data" => {
      let data_url =
        DataUrl::process(url.as_str()).map_err(FetchError::DataUrl)?;

      let (body, _) = data_url.decode_to_vec().map_err(FetchError::Base64)?;
      let body = http_body_util::Full::new(body.into())
        .map_err(|never| match never {})
        .boxed();

      let response = http::Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, data_url.mime_type().to_string())
        .body(body)?;

      let fut = async move { Ok(Ok(response)) };

      let request_rid = state.resource_table.add(FetchRequestResource {
        future: Box::pin(fut),
        url,
      });

      (request_rid, None)
    }
    "blob" => {
      // Blob URL resolution happens in the JS side of fetch. If we got here is
      // because the URL isn't an object URL.
      return Err(FetchError::BlobNotFound);
    }
    _ => return Err(FetchError::SchemeNotSupported(scheme.to_string())),
  };

  Ok(FetchReturn {
    request_rid,
    cancel_handle_rid,
  })
}

#[derive(Default, ToV8)]
pub struct FetchResponse {
  pub status: u16,
  pub status_text: String,
  pub headers: Vec<(ByteString, ByteString)>,
  pub url: String,
  pub response_rid: ResourceId,
  pub content_length: Option<u64>,
  /// This field is populated if some error occurred which needs to be
  /// reconstructed in the JS side to set the error _cause_.
  /// In the tuple, the first element is an error message and the second one is
  /// an error cause.
  pub error: Option<(String, String)>,
}

#[op2]
pub async fn op_fetch_send(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<FetchResponse, FetchError> {
  let request = state
    .borrow_mut()
    .resource_table
    .take::<FetchRequestResource>(rid)?;

  let request = Rc::try_unwrap(request)
    .ok()
    .expect("multiple op_fetch_send ongoing");

  let res = match request.future.await {
    Ok(Ok(res)) => res,
    Ok(Err(err)) => {
      // We're going to try and rescue the error cause from a stream and return it from this fetch.
      // If any error in the chain is a hyper body error, return that as a special result we can use to
      // reconstruct an error chain (eg: `new TypeError(..., { cause: new Error(...) })`).
      // TODO(mmastrac): it would be a lot easier if we just passed a v8::Global through here instead

      if let FetchError::ClientSend(err_src) = &err
        && let Some(client_err) = std::error::Error::source(&err_src.source)
        && let Some(err_src) = client_err.downcast_ref::<hyper::Error>()
        && let Some(err_src) = std::error::Error::source(err_src)
      {
        return Ok(FetchResponse {
          error: Some((err.to_string(), err_src.to_string())),
          ..Default::default()
        });
      }

      return Err(err);
    }
    Err(_) => return Err(FetchError::RequestCanceled),
  };

  let status = res.status();
  let url = request.url.into();
  let mut res_headers = Vec::new();
  for (key, val) in res.headers().iter() {
    res_headers.push((key.as_str().into(), val.as_bytes().into()));
  }

  let content_length = hyper::body::Body::size_hint(res.body()).exact();

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
    error: None,
  })
}

/// Resolves when the response body finishes and rejects (`TypeError`) when the
/// connection errors. Lets `reader.closed`/`read()` settle on a network error
/// even when the consumer isn't actively reading the body.
/// See https://github.com/denoland/deno/issues/16246.
#[op2]
pub async fn op_fetch_response_closed(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(), JsErrorBox> {
  let (resource, tracker) = {
    let state = state.borrow();
    let resource = state
      .resource_table
      .get::<FetchResponseResource>(rid)
      .map_err(JsErrorBox::from_err)?;
    (resource, state.external_ops_tracker.clone())
  };
  resource.closed(tracker).await
}

type CancelableResponseResult =
  Result<Result<http::Response<ResBody>, FetchError>, Canceled>;

pub struct FetchRequestResource {
  pub future: Pin<Box<dyn Future<Output = CancelableResponseResult>>>,
  pub url: Url,
}

impl Resource for FetchRequestResource {
  fn name(&self) -> Cow<'_, str> {
    "fetchRequest".into()
  }
}

pub struct FetchCancelHandle(pub Rc<CancelHandle>);

impl Resource for FetchCancelHandle {
  fn name(&self) -> Cow<'_, str> {
    "fetchCancelHandle".into()
  }

  fn close(self: Rc<Self>) {
    self.0.cancel()
  }
}

type BytesStream =
  Pin<Box<dyn Stream<Item = Result<bytes::Bytes, std::io::Error>> + Unpin>>;

/// Terminal status of a response body once its underlying stream has ended,
/// either cleanly (`Ok`) or with a network error (`Err(message)`).
type BodyTerminal = Result<(), String>;

/// State shared between the background reader task (which is the sole consumer
/// of the raw body stream), the `read` op and the `closed` op.
///
/// Deno's fetch response body is consumed lazily: the underlying network stream
/// is only polled when there is read demand. A hyper connection error, however,
/// is only observed *while reading the body*, so when the consumer only awaits
/// `reader.closed` (or stops after the first chunk) the error would never be
/// noticed and `closed` would hang. To fix that, a background task proactively
/// drives the body stream and records its terminal status here.
/// See https://github.com/denoland/deno/issues/16246.
#[derive(Clone)]
struct BodyReaderShared {
  /// Set once the body stream ends (EOF or error).
  terminal: Rc<RefCell<Option<BodyTerminal>>>,
  /// Partial chunk left over from a previous `read` that requested fewer bytes
  /// than were available.
  leftover: Rc<RefCell<Option<Bytes>>>,
  /// Number of chunks the background task has committed to the channel that
  /// have not yet been consumed by `read`. Incremented *before* send and
  /// decremented *after* recv so it never undercounts: overcounting merely
  /// delays an error `closed` (safe), while undercounting could let `closed`
  /// error the stream while buffered data is still pending.
  buffered: Rc<Cell<usize>>,
  /// Set the first time the consumer issues a `read`. Distinguishes a consumer
  /// that drains the body from one that only awaits `reader.closed`.
  has_read: Rc<Cell<bool>>,
  /// Notified whenever `terminal` or `buffered` changes so a pending `closed`
  /// re-checks `settled_terminal`.
  notify: Rc<tokio::sync::Notify>,
}

impl BodyReaderShared {
  /// The terminal status, if `closed` can settle now.
  ///
  /// A clean termination settles immediately even if buffered data is still
  /// pending (future reads will serve it); settling promptly lets the watcher
  /// op release its reference to the JS stream so an unconsumed response can be
  /// garbage-collected.
  ///
  /// An *error* terminal settles either:
  /// - immediately, if the consumer never issued a `read` (`!has_read`): there
  ///   is no buffered data anyone is waiting to drain, and the resource-backed
  ///   stream has a high-water-mark of 0 so nothing would ever pull it; gating
  ///   on `buffered` here is what would deadlock `await reader.closed` (#16246),
  ///   or
  /// - once all buffered data has been consumed, if the consumer *is* reading,
  ///   so a chunk received just before the connection errored is still
  ///   delivered to the reader before the stream is errored.
  fn settled_terminal(&self) -> Option<BodyTerminal> {
    let terminal = self.terminal.borrow();
    let result = terminal.as_ref()?;
    match result {
      Ok(()) => Some(Ok(())),
      Err(_)
        if !self.has_read.get()
          || (self.buffered.get() == 0 && self.leftover.borrow().is_none()) =>
      {
        Some(result.clone())
      }
      Err(_) => None,
    }
  }
}

/// RAII hold on the event loop via the [`ExternalOpsTracker`]. Refs on
/// construction and unrefs on `release` or drop, so a hold can never leak (and
/// pin the loop alive forever) even if the owning future is dropped.
struct EventLoopHold {
  tracker: ExternalOpsTracker,
  held: bool,
}

impl EventLoopHold {
  fn new(tracker: ExternalOpsTracker) -> Self {
    tracker.ref_op();
    Self {
      tracker,
      held: true,
    }
  }

  fn release(&mut self) {
    if self.held {
      self.tracker.unref_op();
      self.held = false;
    }
  }
}

impl Drop for EventLoopHold {
  fn drop(&mut self) {
    self.release();
  }
}

pub struct BodyReader {
  /// Receives chunks proactively read by the background task.
  rx: tokio::sync::mpsc::Receiver<Bytes>,
  shared: BodyReaderShared,
  /// Background task driving the body stream. Aborted when this `BodyReader`
  /// (and thus the owning resource) is dropped.
  task: Option<deno_core::unsync::JoinHandle<()>>,
}

impl Drop for BodyReader {
  fn drop(&mut self) {
    if let Some(task) = &self.task {
      task.abort();
    }
    // The resource is being torn down (e.g. the body was cancelled/aborted
    // before it finished). Force any `closed` waiter to complete so that the
    // `op_fetch_response_closed` op doesn't leak: mark a clean termination if
    // none was recorded and drop any pending/buffered data.
    if self.shared.terminal.borrow().is_none() {
      *self.shared.terminal.borrow_mut() = Some(Ok(()));
    }
    self.shared.buffered.set(0);
    *self.shared.leftover.borrow_mut() = None;
    self.shared.notify.notify_one();
  }
}

impl BodyReader {
  fn spawn(mut stream: BytesStream) -> BodyReader {
    // A bounded channel of capacity 1 gives backpressure while still allowing a
    // single chunk of read-ahead, which is what lets the background task notice
    // a connection error promptly without buffering the whole body.
    let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(1);
    let shared = BodyReaderShared {
      terminal: Rc::new(RefCell::new(None)),
      leftover: Rc::new(RefCell::new(None)),
      buffered: Rc::new(Cell::new(0)),
      has_read: Rc::new(Cell::new(false)),
      notify: Rc::new(tokio::sync::Notify::new()),
    };
    let task = {
      let shared = shared.clone();
      deno_core::unsync::spawn(async move {
        loop {
          match stream.next().await {
            Some(Ok(chunk)) => {
              if chunk.is_empty() {
                continue;
              }
              // Reserve a buffer slot before sending so an error `closed`
              // never observes an empty buffer while this chunk is still in
              // flight.
              shared.buffered.set(shared.buffered.get() + 1);
              if tx.send(chunk).await.is_err() {
                // The receiver was dropped (resource closed); stop reading.
                shared.buffered.set(shared.buffered.get().saturating_sub(1));
                return;
              }
            }
            Some(Err(err)) => {
              *shared.terminal.borrow_mut() = Some(Err(err.to_string()));
              shared.notify.notify_one();
              return;
            }
            None => {
              *shared.terminal.borrow_mut() = Some(Ok(()));
              shared.notify.notify_one();
              return;
            }
          }
        }
      })
    };
    BodyReader {
      rx,
      shared,
      task: Some(task),
    }
  }
}

pub enum FetchResponseReader {
  Start(http::Response<ResBody>),
  BodyReader(BodyReader),
}

impl Default for FetchResponseReader {
  fn default() -> Self {
    // A terminated reader: the sender half is dropped immediately so reads
    // observe EOF and `terminal` reports a clean close. Only used as a
    // transient placeholder for `mem::take`.
    let (_tx, rx) = tokio::sync::mpsc::channel::<Bytes>(1);
    let shared = BodyReaderShared {
      terminal: Rc::new(RefCell::new(Some(Ok(())))),
      leftover: Rc::new(RefCell::new(None)),
      buffered: Rc::new(Cell::new(0)),
      has_read: Rc::new(Cell::new(false)),
      notify: Rc::new(tokio::sync::Notify::new()),
    };
    Self::BodyReader(BodyReader {
      rx,
      shared,
      task: None,
    })
  }
}

pub struct FetchResponseResource {
  pub response_reader: AsyncRefCell<FetchResponseReader>,
  pub cancel: CancelHandle,
  pub size: Option<u64>,
  /// Handles to the background reader's shared state, mirrored here so the
  /// `closed` op can observe termination without contending for the
  /// `response_reader` borrow that `read` holds across `recv().await`.
  /// `None` until the body reader has been initialized.
  shared: RefCell<Option<BodyReaderShared>>,
}

impl std::fmt::Debug for FetchResponseResource {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    // The body reader holds non-`Debug` handles (a task join handle, channel
    // and `Notify`), so only the cheap, always-available fields are shown.
    f.debug_struct("FetchResponseResource")
      .field("size", &self.size)
      .finish_non_exhaustive()
  }
}

impl FetchResponseResource {
  pub fn new(response: http::Response<ResBody>, size: Option<u64>) -> Self {
    Self {
      response_reader: AsyncRefCell::new(FetchResponseReader::Start(response)),
      cancel: CancelHandle::default(),
      size,
      shared: RefCell::new(None),
    }
  }

  pub async fn upgrade(self) -> Result<hyper::upgrade::Upgraded, hyper::Error> {
    let reader = self.response_reader.into_inner();
    match reader {
      FetchResponseReader::Start(resp) => Ok(hyper::upgrade::on(resp).await?),
      _ => unreachable!(),
    }
  }

  /// Lazily start the background reader task, converting `Start` into
  /// `BodyReader`. Idempotent and cheap once initialized (no async borrow).
  async fn ensure_body_reader(self: Rc<Self>) {
    if self.shared.borrow().is_some() {
      return;
    }
    let mut reader =
      RcRef::map(&self, |r| &r.response_reader).borrow_mut().await;
    if let FetchResponseReader::BodyReader(pipe) = &*reader {
      // Initialized by a racing caller between our check and borrow.
      *self.shared.borrow_mut() = Some(pipe.shared.clone());
      return;
    }
    let FetchResponseReader::Start(resp) = std::mem::take(&mut *reader) else {
      return;
    };
    let stream: BytesStream = Box::pin(
      resp
        .into_body()
        .into_data_stream()
        .map(|r| r.map_err(std::io::Error::other)),
    );
    let body_reader = BodyReader::spawn(stream);
    *self.shared.borrow_mut() = Some(body_reader.shared.clone());
    *reader = FetchResponseReader::BodyReader(body_reader);
  }

  /// Resolves when the body stream finishes cleanly and rejects (with a
  /// `TypeError`) when it errors, even if the consumer never reads the body.
  ///
  /// The op's own promise is unref'd on the JS side; instead it keeps the event
  /// loop alive through `tracker` only while a consumer that has *not started
  /// reading* is waiting for the body to terminate (the `await reader.closed`
  /// with no read case). Once the consumer issues its first read, that hold is
  /// released: the consumer's own read ops then keep the loop alive and drive
  /// the body, and an engaged-then-abandoned body is free to let the process
  /// exit instead of being pinned alive until the peer closes the connection.
  pub async fn closed(
    self: Rc<Self>,
    tracker: ExternalOpsTracker,
  ) -> Result<(), JsErrorBox> {
    // Hold the loop open up-front so the unref'd op promise can't let it drain
    // before the first check below.
    let mut hold = EventLoopHold::new(tracker);
    self.clone().ensure_body_reader().await;
    let shared = self
      .shared
      .borrow()
      .clone()
      .expect("body reader is initialized by ensure_body_reader");
    // Release the resource handle so this op does not keep the resource (and
    // its background task) alive. The op completes either when the body
    // terminates (set by the background task) or when the resource is torn
    // down (set by `BodyReader::drop`).
    drop(self);
    let result = loop {
      if let Some(result) = shared.settled_terminal() {
        break result;
      }
      // The consumer started reading: stop pinning the loop alive (its read ops
      // do that now), so an abandoned body can let the process exit.
      if shared.has_read.get() {
        hold.release();
      }
      shared.notify.notified().await;
    };
    drop(hold);
    match result {
      Ok(()) => Ok(()),
      Err(err) => Err(JsErrorBox::type_error(err)),
    }
  }
}

impl Resource for FetchResponseResource {
  fn name(&self) -> Cow<'_, str> {
    "fetchResponse".into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(async move {
      self.clone().ensure_body_reader().await;

      let cancel_handle = RcRef::map(self.clone(), |r| &r.cancel);
      let fut = async move {
        let mut reader =
          RcRef::map(&self, |r| &r.response_reader).borrow_mut().await;
        let pipe = match &mut *reader {
          FetchResponseReader::BodyReader(pipe) => pipe,
          // `ensure_body_reader` guarantees the `BodyReader` variant.
          FetchResponseReader::Start(_) => unreachable!(),
        };

        // Mark the body as actively read so an error `closed` waits for the
        // buffered chunk to be drained here before the stream is errored.
        pipe.shared.has_read.set(true);

        // Serve any leftover from a previous partial read first.
        {
          let mut leftover = pipe.shared.leftover.borrow_mut();
          if let Some(buf) = leftover.as_mut() {
            let len = min(limit, buf.len());
            let head = buf.split_to(len);
            let drained = buf.is_empty();
            if drained {
              *leftover = None;
            }
            drop(leftover);
            if drained {
              // Wake a pending error `closed` in case draining the leftover
              // just emptied the buffer and opened the gate.
              pipe.shared.notify.notify_one();
            }
            return Ok(BufView::from(head));
          }
        }

        match pipe.rx.recv().await {
          Some(mut chunk) => {
            pipe
              .shared
              .buffered
              .set(pipe.shared.buffered.get().saturating_sub(1));
            let len = min(limit, chunk.len());
            let head = chunk.split_to(len);
            if !chunk.is_empty() {
              *pipe.shared.leftover.borrow_mut() = Some(chunk);
            }
            // Wake a pending error `closed` so it re-checks the gate now that a
            // buffered chunk has been consumed.
            pipe.shared.notify.notify_one();
            Ok(BufView::from(head))
          }
          None => {
            // The channel closed: the body has terminated. Surface a body
            // error here so a pending `read()` rejects too.
            match &*pipe.shared.terminal.borrow() {
              Some(Err(err)) => Err(JsErrorBox::type_error(err.clone())),
              _ => Ok(BufView::empty()),
            }
          }
        }
      };

      fut
        .try_or_cancel(cancel_handle)
        .await
        .map_err(JsErrorBox::from_err)
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
  fn name(&self) -> Cow<'_, str> {
    "httpClient".into()
  }
}

impl HttpClientResource {
  fn new(client: Client, allow_host: bool) -> Self {
    Self { client, allow_host }
  }
}

#[derive(Debug, FromV8)]
pub struct CreateHttpClientArgs {
  ca_certs: Vec<String>,
  #[from_v8(serde)]
  proxy: Option<Proxy>,
  pool_max_idle_per_host: Option<usize>,
  #[from_v8(serde)]
  pool_idle_timeout: Option<serde_json::Value>,
  #[from_v8(default = true)]
  http1: bool,
  #[from_v8(default = true)]
  http2: bool,
  #[from_v8(default)]
  allow_host: bool,
  local_address: Option<String>,
}

#[op2(stack_trace)]
#[smi]
#[allow(clippy::result_large_err, reason = "TODO: investigate")]
pub fn op_fetch_custom_client(
  state: &mut OpState,
  #[scoped] mut args: CreateHttpClientArgs,
  #[cppgc] tls_keys: &TlsKeysHolder,
) -> Result<ResourceId, FetchError> {
  if let Some(proxy) = &mut args.proxy {
    let permissions = state.borrow_mut::<PermissionsContainer>();
    match proxy {
      Proxy::Http { url, .. } => {
        let url = Url::parse(url)?;
        permissions.check_net_url(&url, "Deno.createHttpClient()")?;
      }
      Proxy::Tcp { hostname, port } => {
        permissions
          .check_net(&(hostname, Some(*port)), "Deno.createHttpClient()")?;
      }
      Proxy::Unix {
        path: original_path,
      } => {
        let path = Path::new(original_path);
        let resolved_path = permissions
          .check_open(
            Cow::Borrowed(path),
            OpenAccessKind::ReadWriteNoFollow,
            Some("Deno.createHttpClient()"),
          )?
          .into_path();
        if path != resolved_path {
          *original_path = resolved_path.to_string_lossy().into_owned();
        }
      }
      Proxy::Vsock { cid, port } => {
        let permissions = state.borrow_mut::<PermissionsContainer>();
        permissions.check_net_vsock(*cid, *port, "Deno.createHttpClient()")?;
      }
    }
  }

  let permissions = state.borrow::<PermissionsContainer>().clone();
  let options = state.borrow::<Options>();
  let ca_certs = args
    .ca_certs
    .into_iter()
    .map(|cert| cert.into_bytes())
    .collect::<Vec<_>>();

  let client = create_http_client(
    &options.user_agent,
    CreateHttpClientOptions {
      root_cert_store: options
        .root_cert_store()
        .map_err(HttpClientCreateError::RootCertStore)?,
      ca_certs,
      proxy: args.proxy,
      dns_resolver: dns::Resolver::default().with_permissions(permissions),
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
      local_address: args.local_address,
      client_builder_hook: options.client_builder_hook,
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
  pub dns_resolver: dns::Resolver,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub client_cert_chain_and_key: Option<TlsKey>,
  pub pool_max_idle_per_host: Option<usize>,
  pub pool_idle_timeout: Option<Option<u64>>,
  pub http1: bool,
  pub http2: bool,
  pub local_address: Option<String>,
  pub client_builder_hook: Option<fn(HyperClientBuilder) -> HyperClientBuilder>,
}

impl Default for CreateHttpClientOptions {
  fn default() -> Self {
    CreateHttpClientOptions {
      root_cert_store: None,
      ca_certs: vec![],
      proxy: None,
      dns_resolver: dns::Resolver::default(),
      unsafely_ignore_certificate_errors: None,
      client_cert_chain_and_key: None,
      pool_max_idle_per_host: None,
      pool_idle_timeout: None,
      http1: true,
      http2: true,
      local_address: None,
      client_builder_hook: None,
    }
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
pub enum HttpClientCreateError {
  #[error(transparent)]
  Tls(deno_tls::TlsError),
  #[error("Illegal characters in User-Agent: received {0}")]
  InvalidUserAgent(String),
  #[error("Invalid address: {0}")]
  InvalidAddress(String),
  #[error("invalid proxy url")]
  InvalidProxyUrl,
  #[error(
    "Cannot create Http Client: either `http1` or `http2` needs to be set to true"
  )]
  HttpVersionSelectionInvalid,
  #[class(inherit)]
  #[error(transparent)]
  RootCertStore(JsErrorBox),
  #[error("Unix proxy is not supported on Windows")]
  UnixProxyNotSupportedOnWindows,
  #[error("Vsock proxy is not supported on this platform")]
  VsockProxyNotSupported,
}

/// Create new instance of async Client. This client supports
/// proxies and doesn't follow redirects.
pub fn create_http_client(
  user_agent: &str,
  options: CreateHttpClientOptions,
) -> Result<Client, HttpClientCreateError> {
  let mut tls_config =
    deno_tls::create_client_config(deno_tls::TlsClientConfigOptions {
      root_cert_store: options.root_cert_store,
      ca_certs: options.ca_certs,
      unsafely_ignore_certificate_errors: options
        .unsafely_ignore_certificate_errors,
      unsafely_disable_hostname_verification: false,
      cert_chain_and_key: options.client_cert_chain_and_key.into(),
      socket_use: deno_tls::SocketUse::Http,
    })
    .map_err(HttpClientCreateError::Tls)?;

  // Proxy TLS should not send ALPN
  tls_config.alpn_protocols.clear();
  let proxy_tls_config = Arc::from(tls_config.clone());

  let mut alpn_protocols = vec![];
  if options.http2 {
    alpn_protocols.push("h2".into());
  }
  if options.http1 {
    alpn_protocols.push("http/1.1".into());
  }
  tls_config.alpn_protocols = alpn_protocols;
  let tls_config = Arc::from(tls_config);

  let mut http_connector =
    HttpConnector::new_with_resolver(options.dns_resolver.clone());
  http_connector.enforce_http(false);
  if let Some(local_address) = options.local_address {
    let local_addr = local_address
      .parse::<IpAddr>()
      .map_err(|_| HttpClientCreateError::InvalidAddress(local_address))?;
    http_connector.set_local_address(Some(local_addr));
  }
  let http_connector = dns::PermissionedHttpConnector::new(http_connector);

  let user_agent = user_agent.parse::<HeaderValue>().map_err(|_| {
    HttpClientCreateError::InvalidUserAgent(user_agent.to_string())
  })?;

  let mut builder = HyperClientBuilder::new(TokioExecutor::new());
  builder.timer(TokioTimer::new());
  builder.pool_timer(TokioTimer::new());

  if let Some(client_builder_hook) = options.client_builder_hook {
    builder = client_builder_hook(builder);
  }

  let mut proxies = proxy::from_env();
  if let Some(proxy) = options.proxy {
    let intercept = match proxy {
      Proxy::Http { url, basic_auth } => {
        let target = proxy::Target::parse(&url)
          .ok_or_else(|| HttpClientCreateError::InvalidProxyUrl)?;
        let mut intercept = proxy::Intercept::all(target);
        if let Some(basic_auth) = &basic_auth {
          intercept.set_auth(&basic_auth.username, &basic_auth.password);
        }
        intercept
      }
      Proxy::Tcp {
        hostname: host,
        port,
      } => {
        let target = proxy::Target::new_tcp(host, port);
        proxy::Intercept::all(target)
      }
      #[cfg(not(windows))]
      Proxy::Unix { path } => {
        let target = proxy::Target::new_unix(PathBuf::from(path));
        proxy::Intercept::all(target)
      }
      #[cfg(windows)]
      Proxy::Unix { .. } => {
        return Err(HttpClientCreateError::UnixProxyNotSupportedOnWindows);
      }
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Proxy::Vsock { cid, port } => {
        let target = proxy::Target::new_vsock(cid, port);
        proxy::Intercept::all(target)
      }
      #[cfg(not(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      )))]
      Proxy::Vsock { .. } => {
        return Err(HttpClientCreateError::VsockProxyNotSupported);
      }
    };
    proxies.prepend(intercept);
  }
  let proxies = Arc::new(proxies);
  let connector = proxy::ProxyConnector {
    http: http_connector,
    proxies,
    tls: tls_config,
    tls_proxy: proxy_tls_config,
    user_agent: Some(user_agent.clone()),
  };

  if let Some(pool_max_idle_per_host) = options.pool_max_idle_per_host {
    builder.pool_max_idle_per_host(pool_max_idle_per_host);
  }

  if let Some(pool_idle_timeout) = options.pool_idle_timeout {
    builder.pool_idle_timeout(
      pool_idle_timeout.map(std::time::Duration::from_millis),
    );
  }

  match (options.http1, options.http2) {
    (true, false) => {} // noop, handled by ALPN above
    (false, true) => {
      builder.http2_only(true);
    }
    (true, true) => {}
    (false, false) => {
      return Err(HttpClientCreateError::HttpVersionSelectionInvalid);
    }
  }

  let pooled_client = builder.build(connector.clone());
  let retry_client = retry::Retry::new(FetchRetry, pooled_client);
  let decompress = Decompression::new(retry_client).gzip(true).br(true);

  Ok(Client {
    inner: decompress,
    connector,
    user_agent,
  })
}

#[op2]
pub fn op_utf8_to_byte_string(#[string] input: String) -> ByteString {
  input.into()
}

#[derive(Clone, Debug)]
pub struct Client {
  inner: Decompression<
    retry::Retry<
      FetchRetry,
      hyper_util::client::legacy::Client<Connector, ReqBody>,
    >,
  >,
  connector: Connector,
  user_agent: HeaderValue,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ClientConnectError {
  #[class(type)]
  #[error("HTTP/1.1 not supported by this client")]
  Http1NotSupported,
  #[class(type)]
  #[error("HTTP/2 not supported by this client")]
  Http2NotSupported,
  #[class(generic)]
  #[error(transparent)]
  Connector(BoxError),
}

impl Client {
  pub async fn connect(
    &self,
    uri: Uri,
    socket_use: SocketUse,
  ) -> Result<
    impl tokio::io::AsyncRead
    + tokio::io::AsyncWrite
    + Connection
    + Unpin
    + Send
    + 'static,
    ClientConnectError,
  > {
    let mut connector = match socket_use {
      SocketUse::Http1Only => {
        let Some(connector) = self.connector.clone().h1_only() else {
          return Err(ClientConnectError::Http1NotSupported);
        };
        connector
      }
      SocketUse::Http2Only => {
        let Some(connector) = self.connector.clone().h2_only() else {
          return Err(ClientConnectError::Http2NotSupported);
        };
        connector
      }
      _ => self.connector.clone(),
    };
    let connection = connector
      .call(uri)
      .await
      .map_err(ClientConnectError::Connector)?;
    Ok(TokioIo::new(connection))
  }
}

type Connector = proxy::ProxyConnector<
  dns::PermissionedHttpConnector<HttpConnector<dns::Resolver>>,
>;

#[allow(
  clippy::declare_interior_mutable_const,
  reason = "clippy is wrong here"
)]
const STAR_STAR: HeaderValue = HeaderValue::from_static("*/*");

#[derive(Debug, deno_error::JsError)]
#[class(type)]
pub struct ClientSendError {
  uri: Uri,
  pub source: hyper_util::client::legacy::Error,
}

impl ClientSendError {
  pub fn is_connect_error(&self) -> bool {
    self.source.is_connect()
  }

  fn http_info(&self) -> Option<HttpInfo> {
    let mut exts = Extensions::new();
    self.source.connect_info()?.get_extras(&mut exts);
    exts.remove::<HttpInfo>()
  }
}

impl std::fmt::Display for ClientSendError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    // NOTE: we can use `std::error::Report` instead once it's stabilized.
    let detail = error_reporter::Report::new(&self.source);

    match self.http_info() {
      Some(http_info) => {
        write!(
          f,
          "error sending request from {src} for {uri} ({dst}): {detail}",
          src = http_info.local_addr(),
          uri = self.uri,
          dst = http_info.remote_addr(),
          detail = detail,
        )
      }
      None => {
        write!(
          f,
          "error sending request for url ({uri}): {detail}",
          uri = self.uri,
          detail = detail,
        )
      }
    }
  }
}

impl std::error::Error for ClientSendError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    Some(&self.source)
  }
}

pub trait CommonRequest {
  fn uri(&self) -> &Uri;
  fn headers_mut(&mut self) -> &mut HeaderMap;
}

impl CommonRequest for http::Request<ReqBody> {
  fn uri(&self) -> &Uri {
    self.uri()
  }

  fn headers_mut(&mut self) -> &mut HeaderMap {
    self.headers_mut()
  }
}

impl CommonRequest for http::request::Builder {
  fn uri(&self) -> &Uri {
    http::request::Builder::uri_ref(self).expect("uri not set")
  }

  fn headers_mut(&mut self) -> &mut HeaderMap {
    http::request::Builder::headers_mut(self).expect("headers not set")
  }
}

impl Client {
  /// Injects common headers like User-Agent and Proxy-Authorization.
  pub fn inject_common_headers(&self, req: &mut impl CommonRequest) {
    req
      .headers_mut()
      .entry(USER_AGENT)
      .or_insert_with(|| self.user_agent.clone());

    if let Some(auth) = self.connector.proxies.http_forward_auth(req.uri()) {
      req.headers_mut().insert(PROXY_AUTHORIZATION, auth.clone());
    }
  }

  pub async fn send(
    self,
    mut req: http::Request<ReqBody>,
  ) -> Result<http::Response<ResBody>, ClientSendError> {
    self.inject_common_headers(&mut req);

    req.headers_mut().entry(ACCEPT).or_insert(STAR_STAR);

    let uri = req.uri().clone();

    let resp = self
      .inner
      .oneshot(req)
      .await
      .map_err(|e| ClientSendError { uri, source: e })?;
    Ok(resp.map(|b| b.map_err(|e| JsErrorBox::generic(e.to_string())).boxed()))
  }

  /// Sends a request bypassing the transparent decompression middleware.
  /// The response body will contain raw bytes (potentially compressed).
  /// The caller is responsible for checking Content-Encoding and
  /// decompressing if needed.
  pub async fn send_no_decompress(
    self,
    mut req: http::Request<ReqBody>,
  ) -> Result<http::Response<ResBody>, ClientSendError> {
    self.inject_common_headers(&mut req);

    req.headers_mut().entry(ACCEPT).or_insert(STAR_STAR);

    let uri = req.uri().clone();

    // .into_inner() unwraps the Decompression middleware layer
    let resp = self
      .inner
      .into_inner()
      .oneshot(req)
      .await
      .map_err(|e| ClientSendError { uri, source: e })?;
    Ok(resp.map(|b| b.map_err(|e| JsErrorBox::generic(e.to_string())).boxed()))
  }
}

// This is a custom enum to allow the retry policy to clone the variants that could be retried.
pub enum ReqBody {
  Full(http_body_util::Full<Bytes>),
  Empty(http_body_util::Empty<Bytes>),
  Streaming(BoxBody<Bytes, JsErrorBox>),
}

pub type ResBody = BoxBody<Bytes, JsErrorBox>;

impl ReqBody {
  pub fn full(bytes: Bytes) -> Self {
    ReqBody::Full(http_body_util::Full::new(bytes))
  }

  pub fn empty() -> Self {
    ReqBody::Empty(http_body_util::Empty::new())
  }

  pub fn streaming<B>(body: B) -> Self
  where
    B: hyper::body::Body<Data = Bytes, Error = JsErrorBox>
      + Send
      + Sync
      + 'static,
  {
    ReqBody::Streaming(BoxBody::new(body))
  }
}

impl hyper::body::Body for ReqBody {
  type Data = Bytes;
  type Error = JsErrorBox;

  fn poll_frame(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
    match &mut *self {
      ReqBody::Full(b) => {
        Pin::new(b).poll_frame(cx).map_err(|never| match never {})
      }
      ReqBody::Empty(b) => {
        Pin::new(b).poll_frame(cx).map_err(|never| match never {})
      }
      ReqBody::Streaming(b) => Pin::new(b).poll_frame(cx),
    }
  }

  fn is_end_stream(&self) -> bool {
    match self {
      ReqBody::Full(b) => b.is_end_stream(),
      ReqBody::Empty(b) => b.is_end_stream(),
      ReqBody::Streaming(b) => b.is_end_stream(),
    }
  }

  fn size_hint(&self) -> hyper::body::SizeHint {
    match self {
      ReqBody::Full(b) => b.size_hint(),
      ReqBody::Empty(b) => b.size_hint(),
      ReqBody::Streaming(b) => b.size_hint(),
    }
  }
}

/// Copied from https://github.com/seanmonstar/reqwest/blob/b9d62a0323d96f11672a61a17bf8849baec00275/src/async_impl/request.rs#L572
/// Check the request URL for a "username:password" type authority, and if
/// found, remove it from the URL and return it.
pub fn extract_authority(url: &mut Url) -> Option<(String, Option<String>)> {
  use percent_encoding::percent_decode;

  if url.has_authority() {
    let username: String = percent_decode(url.username().as_bytes())
      .decode_utf8()
      .ok()?
      .into();
    let password = url.password().and_then(|pass| {
      percent_decode(pass.as_bytes())
        .decode_utf8()
        .ok()
        .map(String::from)
    });
    if !username.is_empty() || password.is_some() {
      url
        .set_username("")
        .expect("has_authority means set_username shouldn't fail");
      url
        .set_password(None)
        .expect("has_authority means set_password shouldn't fail");
      return Some((username, password));
    }
  }

  None
}

#[op2(fast)]
fn op_fetch_promise_is_settled(promise: v8::Local<v8::Promise>) -> bool {
  promise.state() != v8::PromiseState::Pending
}

/// Deno.fetch's retry policy.
#[derive(Clone, Debug)]
struct FetchRetry;

/// Marker extension that a request has been retried once.
#[derive(Clone, Debug)]
struct Retried;

impl<ResBody, E>
  retry::Policy<http::Request<ReqBody>, http::Response<ResBody>, E>
  for FetchRetry
where
  E: std::error::Error + 'static,
{
  /// Don't delay retries.
  type Future = future::Ready<()>;

  fn retry(
    &mut self,
    req: &mut http::Request<ReqBody>,
    result: &mut Result<http::Response<ResBody>, E>,
  ) -> Option<Self::Future> {
    if req.extensions().get::<Retried>().is_some() {
      // only retry once
      return None;
    }

    match result {
      Ok(..) => {
        // never retry a Response
        None
      }
      Err(err) => {
        if is_error_retryable(&*err) {
          req.extensions_mut().insert(Retried);
          Some(future::ready(()))
        } else {
          None
        }
      }
    }
  }

  fn clone_request(
    &mut self,
    req: &http::Request<ReqBody>,
  ) -> Option<http::Request<ReqBody>> {
    let body = match req.body() {
      ReqBody::Full(b) => ReqBody::Full(b.clone()),
      ReqBody::Empty(b) => ReqBody::Empty(*b),
      ReqBody::Streaming(..) => return None,
    };

    let mut clone = http::Request::new(body);
    *clone.method_mut() = req.method().clone();
    *clone.uri_mut() = req.uri().clone();
    *clone.headers_mut() = req.headers().clone();
    *clone.extensions_mut() = req.extensions().clone();
    Some(clone)
  }
}

fn is_error_retryable(err: &(dyn std::error::Error + 'static)) -> bool {
  // Note: hyper doesn't promise it will always be this h2 version. Keep up to date.
  if let Some(err) = find_source::<h2::Error>(err) {
    // They sent us a graceful shutdown, try with a new connection!
    if err.is_go_away()
      && err.is_remote()
      && err.reason() == Some(h2::Reason::NO_ERROR)
    {
      return true;
    }

    // REFUSED_STREAM was sent from the server, which is safe to retry.
    // https://www.rfc-editor.org/rfc/rfc9113.html#section-8.7-3.2
    if err.is_reset()
      && err.is_remote()
      && err.reason() == Some(h2::Reason::REFUSED_STREAM)
    {
      return true;
    }
  }

  // HTTP/1.1: The connection was closed before the message completed.
  // This happens when a pooled keep-alive connection is stale (e.g. the
  // server shut down between requests). Safe to retry because the server
  // never received/processed the request on this connection.
  if let Some(err) = find_source::<hyper::Error>(err)
    && err.is_incomplete_message()
  {
    return true;
  }

  // Connection reset/aborted by the server before we could send the request.
  // This is another manifestation of stale pooled connections.
  // ConnectionReset (ECONNRESET) on Unix, ConnectionAborted (WSAECONNABORTED /
  // os error 10053) on Windows.
  if let Some(err) = find_source::<std::io::Error>(err)
    && matches!(
      err.kind(),
      std::io::ErrorKind::ConnectionReset
        | std::io::ErrorKind::ConnectionAborted
    )
  {
    return true;
  }

  false
}

fn find_source<'a, E: std::error::Error + 'static>(
  err: &'a (dyn std::error::Error + 'static),
) -> Option<&'a E> {
  let mut err = Some(err);
  while let Some(src) = err {
    if let Some(found) = src.downcast_ref::<E>() {
      return Some(found);
    }
    err = src.source();
  }
  None
}
