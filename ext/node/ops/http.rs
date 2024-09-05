// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

use bytes::Bytes;
use deno_core::anyhow;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::stream::Peekable;
use deno_core::futures::Future;
use deno_core::futures::FutureExt;
use deno_core::futures::Stream;
use deno_core::futures::StreamExt;
use deno_core::op2;
use deno_core::serde::Serialize;
use deno_core::unsync::spawn;
use deno_core::url::Url;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_fetch::get_or_create_client_from_state;
use deno_fetch::FetchCancelHandle;
use deno_fetch::FetchRequestResource;
use deno_fetch::FetchReturn;
use deno_fetch::HttpClientResource;
use deno_fetch::ResBody;
use http::header::HeaderMap;
use http::header::HeaderName;
use http::header::HeaderValue;
use http::header::AUTHORIZATION;
use http::header::CONTENT_LENGTH;
use http::Method;
use http_body_util::BodyExt;
use hyper::body::Frame;
use hyper_util::rt::TokioIo;
use std::cmp::min;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

#[op2]
#[serde]
pub fn op_node_http_request<P>(
  state: &mut OpState,
  #[serde] method: ByteString,
  #[string] url: String,
  #[serde] headers: Vec<(ByteString, ByteString)>,
  #[smi] client_rid: Option<u32>,
  #[smi] body: Option<ResourceId>,
) -> Result<FetchReturn, AnyError>
where
  P: crate::NodePermissions + 'static,
{
  let client = if let Some(rid) = client_rid {
    let r = state.resource_table.get::<HttpClientResource>(rid)?;
    r.client.clone()
  } else {
    get_or_create_client_from_state(state)?
  };

  let method = Method::from_bytes(&method)?;
  let mut url = Url::parse(&url)?;
  let maybe_authority = deno_fetch::extract_authority(&mut url);

  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_net_url(&url, "ClientRequest")?;
  }

  let mut header_map = HeaderMap::new();
  for (key, value) in headers {
    let name = HeaderName::from_bytes(&key)
      .map_err(|err| type_error(err.to_string()))?;
    let v = HeaderValue::from_bytes(&value)
      .map_err(|err| type_error(err.to_string()))?;

    header_map.append(name, v);
  }

  let (body, con_len) = if let Some(body) = body {
    (
      BodyExt::boxed(NodeHttpResourceToBodyAdapter::new(
        state.resource_table.take_any(body)?,
      )),
      None,
    )
  } else {
    // POST and PUT requests should always have a 0 length content-length,
    // if there is no body. https://fetch.spec.whatwg.org/#http-network-or-cache-fetch
    let len = if matches!(method, Method::POST | Method::PUT) {
      Some(0)
    } else {
      None
    };
    (
      http_body_util::Empty::new()
        .map_err(|never| match never {})
        .boxed(),
      len,
    )
  };

  let mut request = http::Request::new(body);
  *request.method_mut() = method.clone();
  *request.uri_mut() = url
    .as_str()
    .parse()
    .map_err(|_| type_error("Invalid URL"))?;
  *request.headers_mut() = header_map;

  if let Some((username, password)) = maybe_authority {
    request.headers_mut().insert(
      AUTHORIZATION,
      deno_fetch::basic_auth(&username, password.as_deref()),
    );
  }
  if let Some(len) = con_len {
    request.headers_mut().insert(CONTENT_LENGTH, len.into());
  }

  let cancel_handle = CancelHandle::new_rc();
  let cancel_handle_ = cancel_handle.clone();

  let fut = async move {
    client
      .send(request)
      .or_cancel(cancel_handle_)
      .await
      .map(|res| res.map_err(|err| type_error(err.to_string())))
  };

  let request_rid = state.resource_table.add(FetchRequestResource {
    future: Box::pin(fut),
    url,
  });

  let cancel_handle_rid =
    state.resource_table.add(FetchCancelHandle(cancel_handle));

  Ok(FetchReturn {
    request_rid,
    cancel_handle_rid: Some(cancel_handle_rid),
  })
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeHttpFetchResponse {
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
pub async fn op_node_http_fetch_send(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<NodeHttpFetchResponse, AnyError> {
  let request = state
    .borrow_mut()
    .resource_table
    .take::<FetchRequestResource>(rid)?;

  let request = Rc::try_unwrap(request)
    .ok()
    .expect("multiple op_node_http_fetch_send ongoing");

  let res = match request.future.await {
    Ok(Ok(res)) => res,
    Ok(Err(err)) => {
      // We're going to try and rescue the error cause from a stream and return it from this fetch.
      // If any error in the chain is a hyper body error, return that as a special result we can use to
      // reconstruct an error chain (eg: `new TypeError(..., { cause: new Error(...) })`).
      // TODO(mmastrac): it would be a lot easier if we just passed a v8::Global through here instead
      let mut err_ref: &dyn std::error::Error = err.as_ref();
      while let Some(err) = std::error::Error::source(err_ref) {
        if let Some(err) = err.downcast_ref::<hyper::Error>() {
          if let Some(err) = std::error::Error::source(err) {
            return Ok(NodeHttpFetchResponse {
              error: Some(err.to_string()),
              ..Default::default()
            });
          }
        }
        err_ref = err;
      }

      return Err(type_error(err.to_string()));
    }
    Err(_) => return Err(type_error("request was cancelled")),
  };

  let status = res.status();
  let url = request.url.into();
  let mut res_headers = Vec::new();
  for (key, val) in res.headers().iter() {
    res_headers.push((key.as_str().into(), val.as_bytes().into()));
  }

  let content_length = hyper::body::Body::size_hint(res.body()).exact();
  let remote_addr = res
    .extensions()
    .get::<hyper_util::client::legacy::connect::HttpInfo>()
    .map(|info| info.remote_addr());
  let (remote_addr_ip, remote_addr_port) = if let Some(addr) = remote_addr {
    (Some(addr.ip().to_string()), Some(addr.port()))
  } else {
    (None, None)
  };

  let response_rid = state
    .borrow_mut()
    .resource_table
    .add(NodeHttpFetchResponseResource::new(res, content_length));

  Ok(NodeHttpFetchResponse {
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
pub async fn op_node_http_fetch_response_upgrade(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<ResourceId, AnyError> {
  let raw_response = state
    .borrow_mut()
    .resource_table
    .take::<NodeHttpFetchResponseResource>(rid)?;
  let raw_response = Rc::try_unwrap(raw_response)
    .expect("Someone is holding onto NodeHttpFetchResponseResource");

  let (read, write) = tokio::io::duplex(1024);
  let (read_rx, write_tx) = tokio::io::split(read);
  let (mut write_rx, mut read_tx) = tokio::io::split(write);
  let upgraded = raw_response.upgrade().await?;
  {
    // Stage 3: Pump the data
    let (mut upgraded_rx, mut upgraded_tx) =
      tokio::io::split(TokioIo::new(upgraded));

    spawn(async move {
      let mut buf = [0; 1024];
      loop {
        let read = upgraded_rx.read(&mut buf).await?;
        if read == 0 {
          read_tx.shutdown().await?;
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

type BytesStream =
  Pin<Box<dyn Stream<Item = Result<bytes::Bytes, std::io::Error>> + Unpin>>;

pub enum NodeHttpFetchResponseReader {
  Start(http::Response<ResBody>),
  BodyReader(Peekable<BytesStream>),
}

impl Default for NodeHttpFetchResponseReader {
  fn default() -> Self {
    let stream: BytesStream = Box::pin(deno_core::futures::stream::empty());
    Self::BodyReader(stream.peekable())
  }
}

#[derive(Debug)]
pub struct NodeHttpFetchResponseResource {
  pub response_reader: AsyncRefCell<NodeHttpFetchResponseReader>,
  pub cancel: CancelHandle,
  pub size: Option<u64>,
}

impl NodeHttpFetchResponseResource {
  pub fn new(response: http::Response<ResBody>, size: Option<u64>) -> Self {
    Self {
      response_reader: AsyncRefCell::new(NodeHttpFetchResponseReader::Start(
        response,
      )),
      cancel: CancelHandle::default(),
      size,
    }
  }

  pub async fn upgrade(self) -> Result<hyper::upgrade::Upgraded, AnyError> {
    let reader = self.response_reader.into_inner();
    match reader {
      NodeHttpFetchResponseReader::Start(resp) => {
        Ok(hyper::upgrade::on(resp).await?)
      }
      _ => unreachable!(),
    }
  }
}

impl Resource for NodeHttpFetchResponseResource {
  fn name(&self) -> Cow<str> {
    "fetchResponse".into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(async move {
      let mut reader =
        RcRef::map(&self, |r| &r.response_reader).borrow_mut().await;

      let body = loop {
        match &mut *reader {
          NodeHttpFetchResponseReader::BodyReader(reader) => break reader,
          NodeHttpFetchResponseReader::Start(_) => {}
        }

        match std::mem::take(&mut *reader) {
          NodeHttpFetchResponseReader::Start(resp) => {
            let stream: BytesStream =
              Box::pin(resp.into_body().into_data_stream().map(|r| {
                r.map_err(|err| {
                  std::io::Error::new(std::io::ErrorKind::Other, err)
                })
              }));
            *reader =
              NodeHttpFetchResponseReader::BodyReader(stream.peekable());
          }
          NodeHttpFetchResponseReader::BodyReader(_) => unreachable!(),
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

#[allow(clippy::type_complexity)]
pub struct NodeHttpResourceToBodyAdapter(
  Rc<dyn Resource>,
  Option<Pin<Box<dyn Future<Output = Result<BufView, anyhow::Error>>>>>,
);

impl NodeHttpResourceToBodyAdapter {
  pub fn new(resource: Rc<dyn Resource>) -> Self {
    let future = resource.clone().read(64 * 1024);
    Self(resource, Some(future))
  }
}

// SAFETY: we only use this on a single-threaded executor
unsafe impl Send for NodeHttpResourceToBodyAdapter {}
// SAFETY: we only use this on a single-threaded executor
unsafe impl Sync for NodeHttpResourceToBodyAdapter {}

impl Stream for NodeHttpResourceToBodyAdapter {
  type Item = Result<Bytes, anyhow::Error>;

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
          Ok(buf) => {
            this.1 = Some(this.0.clone().read(64 * 1024));
            Poll::Ready(Some(Ok(buf.to_vec().into())))
          }
          Err(err) => Poll::Ready(Some(Err(err))),
        },
      }
    } else {
      Poll::Ready(None)
    }
  }
}

impl hyper::body::Body for NodeHttpResourceToBodyAdapter {
  type Data = Bytes;
  type Error = anyhow::Error;

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

impl Drop for NodeHttpResourceToBodyAdapter {
  fn drop(&mut self) {
    self.0.clone().close()
  }
}
