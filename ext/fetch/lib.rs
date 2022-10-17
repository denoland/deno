// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod fs_fetch_handler;

use data_url::DataUrl;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::stream::Peekable;
use deno_core::futures::Future;
use deno_core::futures::Stream;
use deno_core::futures::StreamExt;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::BufView;
use deno_core::WriteOutcome;

use deno_core::url::Url;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
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
use deno_tls::rustls::RootCertStore;
use deno_tls::Proxy;
use http::header::CONTENT_LENGTH;
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
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::min;
use std::convert::From;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

// Re-export reqwest and data_url
pub use data_url;
pub use reqwest;

pub use fs_fetch_handler::FsFetchHandler;

#[derive(Clone)]
pub struct Options {
  pub user_agent: String,
  pub root_cert_store: Option<RootCertStore>,
  pub proxy: Option<Proxy>,
  pub request_builder_hook: Option<fn(RequestBuilder) -> RequestBuilder>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub client_cert_chain_and_key: Option<(String, String)>,
  pub file_fetch_handler: Rc<dyn FetchHandler>,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      user_agent: "".to_string(),
      root_cert_store: None,
      proxy: None,
      request_builder_hook: None,
      unsafely_ignore_certificate_errors: None,
      client_cert_chain_and_key: None,
      file_fetch_handler: Rc::new(DefaultFileFetchHandler),
    }
  }
}

pub fn init<FP>(options: Options) -> Extension
where
  FP: FetchPermissions + 'static,
{
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
      op_fetch::decl::<FP>(),
      op_fetch_send::decl(),
      op_fetch_custom_client::decl::<FP>(),
    ])
    .state(move |state| {
      state.put::<Options>(options.clone());
      state.put::<reqwest::Client>({
        create_http_client(
          options.user_agent.clone(),
          options.root_cert_store.clone(),
          vec![],
          options.proxy.clone(),
          options.unsafely_ignore_certificate_errors.clone(),
          options.client_cert_chain_and_key.clone(),
        )
        .unwrap()
      });
      Ok(())
    })
    .build()
}

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
  ) -> (
    CancelableResponseFuture,
    Option<FetchRequestBodyResource>,
    Option<Rc<CancelHandle>>,
  );
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
  ) -> (
    CancelableResponseFuture,
    Option<FetchRequestBodyResource>,
    Option<Rc<CancelHandle>>,
  ) {
    let fut = async move {
      Ok(Err(type_error(
        "NetworkError when attempting to fetch resource.",
      )))
    };
    (Box::pin(fut), None, None)
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

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_fetch.d.ts")
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchReturn {
  request_rid: ResourceId,
  request_body_rid: Option<ResourceId>,
  cancel_handle_rid: Option<ResourceId>,
}

#[op]
pub fn op_fetch<FP>(
  state: &mut OpState,
  method: ByteString,
  url: String,
  headers: Vec<(ByteString, ByteString)>,
  client_rid: Option<u32>,
  has_body: bool,
  body_length: Option<u64>,
  data: Option<ZeroCopyBuf>,
) -> Result<FetchReturn, AnyError>
where
  FP: FetchPermissions + 'static,
{
  let client = if let Some(rid) = client_rid {
    let r = state.resource_table.get::<HttpClientResource>(rid)?;
    r.client.clone()
  } else {
    let client = state.borrow::<reqwest::Client>();
    client.clone()
  };

  let method = Method::from_bytes(&method)?;
  let url = Url::parse(&url)?;

  // Check scheme before asking for net permission
  let scheme = url.scheme();
  let (request_rid, request_body_rid, cancel_handle_rid) = match scheme {
    "file" => {
      let path = url.to_file_path().map_err(|_| {
        type_error("NetworkError when attempting to fetch resource.")
      })?;
      let permissions = state.borrow_mut::<FP>();
      permissions.check_read(&path, "fetch()")?;

      if method != Method::GET {
        return Err(type_error(format!(
          "Fetching files only supports the GET method. Received {}.",
          method
        )));
      }

      let Options {
        file_fetch_handler, ..
      } = state.borrow_mut::<Options>();
      let file_fetch_handler = file_fetch_handler.clone();
      let (request, maybe_request_body, maybe_cancel_handle) =
        file_fetch_handler.fetch_file(state, url);
      let request_rid = state.resource_table.add(FetchRequestResource(request));
      let maybe_request_body_rid =
        maybe_request_body.map(|r| state.resource_table.add(r));
      let maybe_cancel_handle_rid = maybe_cancel_handle
        .map(|ch| state.resource_table.add(FetchCancelHandle(ch)));

      (request_rid, maybe_request_body_rid, maybe_cancel_handle_rid)
    }
    "http" | "https" => {
      let permissions = state.borrow_mut::<FP>();
      permissions.check_net_url(&url, "fetch()")?;

      let mut request = client.request(method.clone(), url);

      let request_body_rid = if has_body {
        match data {
          None => {
            // If no body is passed, we return a writer for streaming the body.
            let (tx, rx) = mpsc::channel::<std::io::Result<bytes::Bytes>>(1);

            // If the size of the body is known, we include a content-length
            // header explicitly.
            if let Some(body_size) = body_length {
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
        // POST and PUT requests should always have a 0 length content-length,
        // if there is no body. https://fetch.spec.whatwg.org/#http-network-or-cache-fetch
        if matches!(method, Method::POST | Method::PUT) {
          request = request.header(CONTENT_LENGTH, HeaderValue::from(0));
        }
        None
      };

      let mut header_map = HeaderMap::new();
      for (key, value) in headers {
        let name = HeaderName::from_bytes(&key)
          .map_err(|err| type_error(err.to_string()))?;
        let v = HeaderValue::from_bytes(&value)
          .map_err(|err| type_error(err.to_string()))?;

        if !matches!(name, HOST | CONTENT_LENGTH) {
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
  content_length: Option<u64>,
}

#[op]
pub async fn op_fetch_send(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
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
    res_headers.push((key.as_str().into(), val.as_bytes().into()));
  }

  let content_length = res.content_length();

  let stream: BytesStream = Box::pin(res.bytes_stream().map(|r| {
    r.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
  }));
  let rid = state
    .borrow_mut()
    .resource_table
    .add(FetchResponseBodyResource {
      reader: AsyncRefCell::new(stream.peekable()),
      cancel: CancelHandle::default(),
      size: content_length,
    });

  Ok(FetchResponse {
    status: status.as_u16(),
    status_text: status.canonical_reason().unwrap_or("").to_string(),
    headers: res_headers,
    url,
    response_rid: rid,
    content_length,
  })
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

pub struct FetchRequestBodyResource {
  body: AsyncRefCell<mpsc::Sender<std::io::Result<bytes::Bytes>>>,
  cancel: CancelHandle,
}

impl Resource for FetchRequestBodyResource {
  fn name(&self) -> Cow<str> {
    "fetchRequestBody".into()
  }

  fn write(self: Rc<Self>, buf: BufView) -> AsyncResult<WriteOutcome> {
    Box::pin(async move {
      let bytes: bytes::Bytes = buf.into();
      let nwritten = bytes.len();
      let body = RcRef::map(&self, |r| &r.body).borrow_mut().await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      body.send(Ok(bytes)).or_cancel(cancel).await?.map_err(|_| {
        type_error("request body receiver not connected (request closed)")
      })?;
      Ok(WriteOutcome::Full { nwritten })
    })
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel()
  }
}

type BytesStream =
  Pin<Box<dyn Stream<Item = Result<bytes::Bytes, std::io::Error>> + Unpin>>;

struct FetchResponseBodyResource {
  reader: AsyncRefCell<Peekable<BytesStream>>,
  cancel: CancelHandle,
  size: Option<u64>,
}

impl Resource for FetchResponseBodyResource {
  fn name(&self) -> Cow<str> {
    "fetchResponseBody".into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(async move {
      let reader = RcRef::map(&self, |r| &r.reader).borrow_mut().await;

      let fut = async move {
        let mut reader = Pin::new(reader);
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
    (0, self.size)
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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateHttpClientOptions {
  ca_certs: Vec<String>,
  proxy: Option<Proxy>,
  cert_chain: Option<String>,
  private_key: Option<String>,
}

#[op]
pub fn op_fetch_custom_client<FP>(
  state: &mut OpState,
  args: CreateHttpClientOptions,
) -> Result<ResourceId, AnyError>
where
  FP: FetchPermissions + 'static,
{
  if let Some(proxy) = args.proxy.clone() {
    let permissions = state.borrow_mut::<FP>();
    let url = Url::parse(&proxy.url)?;
    permissions.check_net_url(&url, "Deno.createHttpClient()")?;
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

  let options = state.borrow::<Options>();
  let ca_certs = args
    .ca_certs
    .into_iter()
    .map(|cert| cert.into_bytes())
    .collect::<Vec<_>>();

  let client = create_http_client(
    options.user_agent.clone(),
    options.root_cert_store.clone(),
    ca_certs,
    args.proxy,
    options.unsafely_ignore_certificate_errors.clone(),
    client_cert_chain_and_key,
  )?;

  let rid = state.resource_table.add(HttpClientResource::new(client));
  Ok(rid)
}

/// Create new instance of async reqwest::Client. This client supports
/// proxies and doesn't follow redirects.
pub fn create_http_client(
  user_agent: String,
  root_cert_store: Option<RootCertStore>,
  ca_certs: Vec<Vec<u8>>,
  proxy: Option<Proxy>,
  unsafely_ignore_certificate_errors: Option<Vec<String>>,
  client_cert_chain_and_key: Option<(String, String)>,
) -> Result<Client, AnyError> {
  let mut tls_config = deno_tls::create_client_config(
    root_cert_store,
    ca_certs,
    unsafely_ignore_certificate_errors,
    client_cert_chain_and_key,
  )?;

  tls_config.alpn_protocols = vec!["h2".into(), "http/1.1".into()];

  let mut headers = HeaderMap::new();
  headers.insert(USER_AGENT, user_agent.parse().unwrap());
  let mut builder = Client::builder()
    .redirect(Policy::none())
    .default_headers(headers)
    .use_preconfigured_tls(tls_config);

  if let Some(proxy) = proxy {
    let mut reqwest_proxy = reqwest::Proxy::all(&proxy.url)?;
    if let Some(basic_auth) = &proxy.basic_auth {
      reqwest_proxy =
        reqwest_proxy.basic_auth(&basic_auth.username, &basic_auth.password);
    }
    builder = builder.proxy(reqwest_proxy);
  }

  // unwrap here because it can only fail when native TLS is used.
  Ok(builder.build().unwrap())
}
