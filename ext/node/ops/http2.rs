// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bytes::Bytes;
use deno_core::error::AnyError;
use deno_core::futures::future::poll_fn;
use deno_core::op;
use deno_core::serde::Serialize;
use deno_core::AsyncRefCell;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use h2;
use http;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use tokio::net::TcpStream;
use url::Url;

pub struct Http2Client {
  pub client: AsyncRefCell<h2::client::SendRequest<Bytes>>,
  pub url: Url,
}

impl Resource for Http2Client {
  fn name(&self) -> Cow<str> {
    "http2Client".into()
  }
}

#[derive(Debug)]
pub struct Http2ClientConn {
  pub conn: AsyncRefCell<h2::client::Connection<TcpStream>>,
  cancel_handle: CancelHandle,
}

impl Resource for Http2ClientConn {
  fn name(&self) -> Cow<str> {
    "http2ClientConnection".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel()
  }
}

#[derive(Debug)]
pub struct Http2ClientStream {
  pub response: AsyncRefCell<h2::client::ResponseFuture>,
  pub stream: AsyncRefCell<h2::SendStream<Bytes>>,
}

impl Resource for Http2ClientStream {
  fn name(&self) -> Cow<str> {
    "http2ClientStream".into()
  }
}

#[derive(Debug)]
pub struct Http2ClientResponseBody {
  pub body: AsyncRefCell<h2::RecvStream>,
}

impl Resource for Http2ClientResponseBody {
  fn name(&self) -> Cow<str> {
    "http2ClientResponseBody".into()
  }
}

#[op]
pub async fn op_http2_connect<P>(
  state: Rc<RefCell<OpState>>,
  url: String,
) -> Result<(ResourceId, ResourceId), AnyError>
where
  P: crate::NodePermissions + 'static,
{
  // TODO(bartlomieju): permission checks
  let url = Url::parse(&url)?;
  // TODO(bartlomieju): handle urls gracefully
  let ip = format!("{}:{}", url.host_str().unwrap(), url.port().unwrap());
  let tcp = TcpStream::connect(ip).await?;
  let (client, conn) = h2::client::handshake(tcp).await?;
  let mut state = state.borrow_mut();
  let client_rid = state.resource_table.add(Http2Client {
    client: AsyncRefCell::new(client),
    url,
  });
  let conn_rid = state.resource_table.add(Http2ClientConn {
    conn: AsyncRefCell::new(conn),
    cancel_handle: CancelHandle::new(),
  });
  Ok((client_rid, conn_rid))
}

#[op]
pub async fn op_http2_poll_client_connection(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError> {
  let resource = state.borrow().resource_table.get::<Http2ClientConn>(rid)?;

  let cancel_handle = RcRef::map(resource.clone(), |this| &this.cancel_handle);
  let mut conn = RcRef::map(resource, |this| &this.conn).borrow_mut().await;

  match (&mut *conn).or_cancel(cancel_handle).await {
    Ok(result) => result?,
    Err(_) => {
      // TODO(bartlomieju): probably need a better mechanism for closing the connection

      // cancelled
    }
  }

  Ok(())
}

#[op]
pub async fn op_http2_client_request(
  state: Rc<RefCell<OpState>>,
  client_rid: ResourceId,
  // TODO(bartlomieju): maybe use a vector with fixed layout to save sending
  // 4 strings of keys?
  mut pseudo_headers: HashMap<String, String>,
  headers: Vec<(ByteString, ByteString)>,
) -> Result<(ResourceId, u32), AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<Http2Client>(client_rid)?;

  let url = resource.url.clone();

  let pseudo_path = pseudo_headers.remove(":path").unwrap_or("/".to_string());
  let pseudo_method = pseudo_headers
    .remove(":method")
    .unwrap_or("GET".to_string());
  // TODO(bartlomieju): handle all pseudo-headers (:authority, :scheme)
  let _pseudo_authority = pseudo_headers
    .remove(":authority")
    .unwrap_or("/".to_string());
  let _pseudo_scheme = pseudo_headers
    .remove(":scheme")
    .unwrap_or("http".to_string());

  let url = url.join(&pseudo_path)?;

  let mut req = http::Request::builder()
    .uri(url.as_str())
    .method(pseudo_method.as_str());

  for (name, value) in headers {
    req.headers_mut().unwrap().append(
      HeaderName::from_lowercase(&name).unwrap(),
      HeaderValue::from_bytes(&value).unwrap(),
    );
  }

  let request = req.body(()).unwrap();

  let resource = {
    let state = state.borrow();
    state.resource_table.get::<Http2Client>(client_rid)?
  };
  let mut client = RcRef::map(&resource, |r| &r.client).borrow_mut().await;
  poll_fn(|cx| client.poll_ready(cx)).await?;
  let (response, stream) = client.send_request(request, false).unwrap();
  let stream_id = stream.stream_id();
  let stream_rid = state.borrow_mut().resource_table.add(Http2ClientStream {
    response: AsyncRefCell::new(response),
    stream: AsyncRefCell::new(stream),
  });
  Ok((stream_rid, stream_id.into()))
}

#[op]
pub async fn op_http2_client_send_data(
  state: Rc<RefCell<OpState>>,
  stream_rid: ResourceId,
  data: JsBuffer,
) -> Result<(), AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<Http2ClientStream>(stream_rid)?;
  let mut stream = RcRef::map(&resource, |r| &r.stream).borrow_mut().await;

  // TODO(bartlomieju): handle end of stream
  stream.send_data(bytes::Bytes::from(data), false)?;
  Ok(())
}

#[op]
pub async fn op_http2_client_end_stream(
  state: Rc<RefCell<OpState>>,
  stream_rid: ResourceId,
) -> Result<(), AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<Http2ClientStream>(stream_rid)?;
  let mut stream = RcRef::map(&resource, |r| &r.stream).borrow_mut().await;

  // TODO(bartlomieju): handle end of stream
  stream.send_data(bytes::Bytes::from(vec![]), true)?;
  Ok(())
}

#[op]
pub async fn op_http2_client_reset_stream(
  state: Rc<RefCell<OpState>>,
  stream_rid: ResourceId,
  code: u32,
) -> Result<(), AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<Http2ClientStream>(stream_rid)?;
  let mut stream = RcRef::map(&resource, |r| &r.stream).borrow_mut().await;
  stream.send_reset(h2::Reason::from(code));
  Ok(())
}

#[op]
pub async fn op_http2_client_send_trailers(
  state: Rc<RefCell<OpState>>,
  stream_rid: ResourceId,
  trailers: Vec<(ByteString, ByteString)>,
) -> Result<(), AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<Http2ClientStream>(stream_rid)?;
  let mut stream = RcRef::map(&resource, |r| &r.stream).borrow_mut().await;

  let mut trailers_map = http::HeaderMap::new();
  for (name, value) in trailers {
    trailers_map.insert(
      HeaderName::from_bytes(&name).unwrap(),
      HeaderValue::from_bytes(&value).unwrap(),
    );
  }

  stream.send_trailers(trailers_map)?;
  Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Http2ClientResponse {
  headers: Vec<(ByteString, ByteString)>,
  body_rid: ResourceId,
  status_code: u16,
}

#[op]
pub async fn op_http2_client_get_response(
  state: Rc<RefCell<OpState>>,
  stream_rid: ResourceId,
) -> Result<Http2ClientResponse, AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<Http2ClientStream>(stream_rid)?;
  let mut response_future =
    RcRef::map(&resource, |r| &r.response).borrow_mut().await;

  let response = (&mut *response_future).await?;

  let (parts, body) = response.into_parts();
  let status = parts.status;
  let mut res_headers = Vec::new();
  for (key, val) in parts.headers.iter() {
    res_headers.push((key.as_str().into(), val.as_bytes().into()));
  }

  let body_rid =
    state
      .borrow_mut()
      .resource_table
      .add(Http2ClientResponseBody {
        body: AsyncRefCell::new(body),
      });
  Ok(Http2ClientResponse {
    headers: res_headers,
    body_rid,
    status_code: status.into(),
  })
}

#[op]
pub async fn op_http2_client_get_response_body_chunk(
  state: Rc<RefCell<OpState>>,
  body_rid: ResourceId,
) -> Result<(Option<Vec<u8>>, bool), AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<Http2ClientResponseBody>(body_rid)?;
  let mut body = RcRef::map(&resource, |r| &r.body).borrow_mut().await;

  let maybe_data = match body.data().await {
    Some(maybe_data) => {
      let data = maybe_data?;
      Some(data.to_vec())
    }
    None => None,
  };

  Ok((maybe_data, body.is_end_stream()))
}

#[op]
pub async fn op_http2_client_get_response_trailers(
  state: Rc<RefCell<OpState>>,
  body_rid: ResourceId,
) -> Result<Option<Vec<(ByteString, ByteString)>>, AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<Http2ClientResponseBody>(body_rid)?;
  let mut body = RcRef::map(&resource, |r| &r.body).borrow_mut().await;
  let maybe_trailers = body.trailers().await?;
  let trailers = if let Some(trailers) = maybe_trailers {
    let mut v = Vec::with_capacity(trailers.len());
    for (key, value) in trailers.iter() {
      v.push((
        ByteString::from(key.as_str()),
        ByteString::from(value.as_bytes()),
      ));
    }
    Some(v)
  } else {
    None
  };
  Ok(trailers)
}
