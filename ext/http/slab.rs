// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::request_properties::HttpConnectionProperties;
use crate::response_body::CompletionHandle;
use crate::response_body::ResponseBytes;
use deno_core::error::AnyError;
use deno_core::OpState;
use deno_core::ResourceId;
use http::request::Parts;
use http::HeaderMap;
use hyper1::body::Incoming;
use hyper1::upgrade::OnUpgrade;

use scopeguard::guard;
use scopeguard::ScopeGuard;
use std::cell::Ref;
use std::cell::RefCell;
use std::cell::RefMut;
use std::rc::Rc;

pub type Request = hyper1::Request<Incoming>;
pub type Response = hyper1::Response<ResponseBytes>;

macro_rules! http_trace {
  ($record:expr, $args:tt) => {
    #[cfg(feature = "__http_tracing")]
    {
      println!(
        "HTTP id={:p} strong={}: {}",
        $record,
        std::rc::Rc::strong_count(&$record),
        format!($args),
      );
    }
  };
}

pub(crate) use http_trace;

#[repr(transparent)]
#[derive(Clone, Default)]
pub struct RefCount(pub Rc<()>);

enum RequestBodyState {
  Incoming(Incoming),
  Resource(HttpRequestBodyAutocloser),
}

impl From<Incoming> for RequestBodyState {
  fn from(value: Incoming) -> Self {
    RequestBodyState::Incoming(value)
  }
}

/// Ensures that the request body closes itself when no longer needed.
pub struct HttpRequestBodyAutocloser(ResourceId, Rc<RefCell<OpState>>);

impl HttpRequestBodyAutocloser {
  pub fn new(res: ResourceId, op_state: Rc<RefCell<OpState>>) -> Self {
    Self(res, op_state)
  }
}

impl Drop for HttpRequestBodyAutocloser {
  fn drop(&mut self) {
    if let Ok(res) = self.1.borrow_mut().resource_table.take_any(self.0) {
      res.close();
    }
  }
}

pub async fn handle_request(
  request: Request,
  request_info: HttpConnectionProperties,
  _refcount: RefCount, // Keep server alive for duration of this future.
  tx: tokio::sync::mpsc::Sender<Rc<HttpRecord>>,
) -> Result<Response, hyper::Error> {
  // If the underlying TCP connection is closed, this future will be dropped
  // and execution could stop at any await point.
  // The HttpRecord must live until JavaScript is done processing so is wrapped
  // in an Rc. The guard ensures unneeded resources are freed at cancellation.
  let guarded_record =
    guard(HttpRecord::new(request, request_info), HttpRecord::cancel);

  // Clone HttpRecord and send to JavaScript for processing.
  // Safe to unwrap as channel receiver is never closed.
  tx.send(guarded_record.clone()).await.unwrap();

  // Wait for JavaScript handler to return request.
  http_trace!(*guarded_record, "handle_request promise.await");
  guarded_record.promise().await;

  // Defuse the guard. Must not await after the point.
  let record = ScopeGuard::into_inner(guarded_record);
  http_trace!(record, "handle_request complete");
  assert!(
    Rc::strong_count(&record) == 1,
    "HTTP state error: Expected to be last strong reference (handle_request)"
  );
  let response = record.take_response();
  Ok(response)
}

struct HttpRecordInner {
  request_info: HttpConnectionProperties,
  request_parts: Parts,
  request_body: Option<RequestBodyState>,
  /// The response may get taken before we tear this down
  response: Option<Response>,
  promise: CompletionHandle,
  trailers: Rc<RefCell<Option<HeaderMap>>>,
  been_dropped: bool,
}

pub struct HttpRecord(RefCell<HttpRecordInner>);

impl HttpRecord {
  fn new(request: Request, request_info: HttpConnectionProperties) -> Rc<Self> {
    let (request_parts, request_body) = request.into_parts();
    let body = ResponseBytes::default();
    let trailers = body.trailers();
    let request_body = Some(request_body.into());
    let inner = HttpRecordInner {
      request_info,
      request_parts,
      request_body,
      response: Some(Response::new(body)),
      trailers,
      been_dropped: false,
      promise: CompletionHandle::default(),
    };
    #[allow(clippy::let_and_return)]
    let record = Rc::new(Self(RefCell::new(inner)));
    http_trace!(record, "HttpRecord::new");
    record
  }

  fn self_ref(&self) -> Ref<'_, HttpRecordInner> {
    self.0.borrow()
  }

  fn self_mut(&self) -> RefMut<'_, HttpRecordInner> {
    self.0.borrow_mut()
  }

  /// Perform the Hyper upgrade on this record.
  pub fn upgrade(&self) -> Result<OnUpgrade, AnyError> {
    // Manually perform the upgrade. We're peeking into hyper's underlying machinery here a bit
    self
      .self_mut()
      .request_parts
      .extensions
      .remove::<OnUpgrade>()
      .ok_or_else(|| AnyError::msg("upgrade unavailable"))
  }

  /// Take the Hyper body from this record.
  pub fn take_body(&self) -> Option<Incoming> {
    let body_holder = &mut self.self_mut().request_body;
    let body = body_holder.take();
    match body {
      Some(RequestBodyState::Incoming(body)) => Some(body),
      x => {
        *body_holder = x;
        None
      }
    }
  }

  pub fn take_resource(&self) -> Option<HttpRequestBodyAutocloser> {
    let body_holder = &mut self.self_mut().request_body;
    let body = body_holder.take();
    match body {
      Some(RequestBodyState::Resource(res)) => Some(res),
      x => {
        *body_holder = x;
        None
      }
    }
  }

  /// Replace the request body with a resource ID and the OpState we'll need to shut it down.
  /// We cannot keep just the resource itself, as JS code might be reading from the resource ID
  /// to generate the response data (requiring us to keep it in the resource table).
  pub fn put_resource(&self, res: HttpRequestBodyAutocloser) {
    self.self_mut().request_body = Some(RequestBodyState::Resource(res));
  }

  /// Cleanup resources not needed after the future is dropped.
  fn cancel(self: Rc<Self>) {
    http_trace!(self, "HttpRecord::cancel");
    let mut inner = self.0.borrow_mut();
    inner.been_dropped = true;
    // The request body might include actual resources.
    inner.request_body.take();
  }

  /// Complete this record, potentially expunging it if it is fully complete (ie: cancelled as well).
  pub fn complete(self: Rc<Self>) {
    http_trace!(self, "HttpRecord::complete");
    let inner = self.self_mut();
    assert!(
      !inner.been_dropped || Rc::strong_count(&self) == 1,
      "HTTP state error: Expected to be last strong reference (been_dropped)"
    );
    assert!(
      !inner.promise.is_completed(),
      "HTTP state error: Entry has already been completed"
    );
    inner.promise.complete(true);
  }

  /// Has the future for this record been dropped? ie, has the underlying TCP connection
  /// been closed?
  pub fn cancelled(&self) -> bool {
    self.self_ref().been_dropped
  }

  /// Get a mutable reference to the response.
  pub fn response(&self) -> RefMut<'_, Response> {
    RefMut::map(self.self_mut(), |inner| inner.response.as_mut().unwrap())
  }

  /// Get a mutable reference to the trailers.
  pub fn trailers(&self) -> Ref<'_, Rc<RefCell<Option<HeaderMap>>>> {
    Ref::map(self.self_ref(), |inner| &inner.trailers)
  }

  /// Take the response.
  fn take_response(&self) -> Response {
    self.self_mut().response.take().unwrap()
  }

  /// Get a reference to the connection properties.
  pub fn request_info(&self) -> Ref<'_, HttpConnectionProperties> {
    Ref::map(self.self_ref(), |inner| &inner.request_info)
  }

  /// Get a reference to the request parts.
  pub fn request_parts(&self) -> Ref<'_, Parts> {
    Ref::map(self.self_ref(), |inner| &inner.request_parts)
  }

  /// Get a reference to the completion handle.
  pub fn promise(&self) -> CompletionHandle {
    self.self_ref().promise.clone()
  }

  /// Get a reference to the response body completion handle.
  pub fn body_promise(&self) -> CompletionHandle {
    self
      .self_ref()
      .response
      .as_ref()
      .unwrap()
      .body()
      .completion_handle()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::hyper_util_tokioio::TokioIo;
  use crate::response_body::Compression;
  use crate::response_body::ResponseBytesInner;
  use bytes::Buf;
  use deno_net::raw::NetworkStreamType;
  use hyper1::body::Body;
  use hyper1::service::service_fn;
  use hyper1::service::HttpService;
  use std::error::Error as StdError;

  /// Execute client request on service and concurrently map the response.
  async fn serve_request<B, S, T, F>(
    req: http::Request<B>,
    service: S,
    map_response: impl FnOnce(hyper1::Response<Incoming>) -> F,
  ) -> hyper1::Result<T>
  where
    B: Body + Send + 'static, // Send bound due to DuplexStream
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    S: HttpService<Incoming>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    S::ResBody: 'static,
    <S::ResBody as Body>::Error: Into<Box<dyn StdError + Send + Sync>>,
    F: std::future::Future<Output = hyper1::Result<T>>,
  {
    use hyper1::client::conn::http1::handshake;
    use hyper1::server::conn::http1::Builder;
    let (stream_client, stream_server) = tokio::io::duplex(16 * 1024);
    let conn_server =
      Builder::new().serve_connection(TokioIo::new(stream_server), service);
    let (mut sender, conn_client) =
      handshake(TokioIo::new(stream_client)).await?;

    let (res, _, _) = tokio::try_join!(
      async move {
        let res = sender.send_request(req).await?;
        map_response(res).await
      },
      conn_server,
      conn_client,
    )?;
    Ok(res)
  }

  #[tokio::test]
  async fn test_handle_request() -> Result<(), AnyError> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    let refcount = RefCount::default();
    let refcount_check = refcount.clone();
    let request_info = HttpConnectionProperties {
      peer_address: "".into(),
      peer_port: None,
      local_port: None,
      stream_type: NetworkStreamType::Tcp,
    };
    let svc = service_fn(move |req: hyper1::Request<Incoming>| {
      handle_request(req, request_info.clone(), refcount.clone(), tx.clone())
    });

    let client_req = http::Request::builder().uri("/").body("".to_string())?;

    // Response produced by concurrent tasks
    tokio::try_join!(
      async move {
        // JavaScript handler produces response
        let record = rx.recv().await.unwrap();
        let resource = record.take_resource();
        record.response().body_mut().initialize(
          ResponseBytesInner::from_vec(
            Compression::None,
            b"hello world".to_vec(),
          ),
          resource,
        );
        record.complete();
        Ok(())
      },
      // Server connection executes service
      async move {
        serve_request(client_req, svc, |res| async {
          // Client reads the response
          use http_body_util::BodyExt;
          assert_eq!(res.status(), 200);
          let body = res.collect().await?.to_bytes();
          assert_eq!(body.chunk(), b"hello world");
          Ok(())
        })
        .await
      },
    )?;
    assert_eq!(Rc::strong_count(&refcount_check.0), 1);
    Ok(())
  }
}
