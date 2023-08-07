// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde::Serialize;
use deno_core::AsyncRefCell;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
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
  // TODO(bartlomieju): handle urls
  let url = Url::parse(&url)?;
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
  headers: Vec<(ByteString, ByteString)>,
  end_of_stream: bool,
) -> Result<ResourceId, AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<Http2Client>(client_rid)?;

  let url = resource.url.clone();

  let mut seen_pseudo_path = false;
  let mut seen_pseudo_method = false;
  let mut pseudo_method = "GET".to_string();
  let mut pseudo_path = "/".to_string();

  // TODO: pseudo-headers should be passed as a separate argument
  // TODO: handle all pseudo-headers (:authority, :scheme)
  // TODO: remove clone
  for (name, value) in headers.clone() {
    if name == ":path".into() {
      seen_pseudo_path = true;
      pseudo_path = String::from_utf8(value.to_vec())?;
      continue;
    }

    if name == ":method".into() {
      seen_pseudo_method = true;
      pseudo_method = String::from_utf8(value.to_vec())?;
      continue;
    }

    if seen_pseudo_method && seen_pseudo_path {
      break;
    }
  }
  let url = url.join(&pseudo_path)?;

  let mut req = http::Request::builder()
    .uri(url.as_str())
    .method(pseudo_method.as_str());

  for (name, value) in headers {
    if name == ":path".into() || name == ":method".into() {
      continue;
    }
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
  let (response, stream) = client.send_request(request, end_of_stream).unwrap();
  let stream_rid = state.borrow_mut().resource_table.add(Http2ClientStream {
    response: AsyncRefCell::new(response),
    stream: AsyncRefCell::new(stream),
  });
  Ok(stream_rid)
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

  // TODO(bartlomieju): maybe this should be done from JS?
  let mut finished = false;
  if body.is_end_stream() {
    let _ = state.borrow_mut().resource_table.close(body_rid);
    finished = true;
  }

  Ok((maybe_data, finished))
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
