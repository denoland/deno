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
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use h2;
use http;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use tokio::net::TcpStream;

pub struct NodeHttp2Client {
  pub client: AsyncRefCell<h2::client::SendRequest<Bytes>>,
}

impl Resource for NodeHttp2Client {
  fn name(&self) -> Cow<str> {
    "nodeHttp2Client".into()
  }
}

#[derive(Debug)]
pub struct NodeHttp2ClientConn {
  pub conn: h2::client::Connection<TcpStream>,
}

impl Resource for NodeHttp2ClientConn {
  fn name(&self) -> Cow<str> {
    "nodeHttp2ClientConnection".into()
  }
}

#[derive(Debug)]
pub struct NodeHttp2ClientStream {
  pub response: AsyncRefCell<h2::client::ResponseFuture>,
  pub stream: AsyncRefCell<h2::SendStream<Bytes>>,
}

impl Resource for NodeHttp2ClientStream {
  fn name(&self) -> Cow<str> {
    "nodeHttp2ClientStream".into()
  }
}

#[derive(Debug)]
pub struct NodeHttp2ClientResponseBody {
  pub body: AsyncRefCell<h2::RecvStream>,
}

impl Resource for NodeHttp2ClientResponseBody {
  fn name(&self) -> Cow<str> {
    "nodeHttp2ClientResponseBody".into()
  }
}

#[op]
pub async fn op_node_http2_client_connect<P>(
  state: Rc<RefCell<OpState>>,
) -> Result<(ResourceId, ResourceId), AnyError>
where
  P: crate::NodePermissions + 'static,
{
  // TODO(bartlomieju): handle urls
  let tcp = TcpStream::connect("localhost:8443").await?;
  let (client, conn) = h2::client::handshake(tcp).await?;
  let mut state = state.borrow_mut();
  let client_rid = state.resource_table.add(NodeHttp2Client {
    client: AsyncRefCell::new(client),
  });
  let conn_rid = state.resource_table.add(NodeHttp2ClientConn { conn });
  Ok((client_rid, conn_rid))
}

#[op]
pub async fn op_node_http2_client_poll_connection(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError> {
  let conn = {
    let mut state = state.borrow_mut();
    let conn = state.resource_table.take::<NodeHttp2ClientConn>(rid)?;
    Rc::try_unwrap(conn).unwrap()
  };

  conn.conn.await?;

  Ok(())
}

#[op]
pub async fn op_node_http2_client_request(
  state: Rc<RefCell<OpState>>,
  client_rid: ResourceId,
  headers: Vec<(ByteString, ByteString)>,
  end_of_stream: bool,
) -> Result<ResourceId, AnyError> {
  // TODO(bartlomieju): handle URL
  let mut req = http::Request::builder().uri("http://localhost:8443");

  for (name, value) in headers {
    req.headers_mut().unwrap().append(
      HeaderName::from_lowercase(&name).unwrap(),
      HeaderValue::from_bytes(&value).unwrap(),
    );
  }

  let request = req.body(()).unwrap();

  let resource = {
    let state = state.borrow();
    state.resource_table.get::<NodeHttp2Client>(client_rid)?
  };
  let mut client = RcRef::map(&resource, |r| &r.client).borrow_mut().await;
  let (response, stream) = client.send_request(request, end_of_stream).unwrap();
  let stream_rid = {
    let mut state = state.borrow_mut();
    state.resource_table.add(NodeHttp2ClientStream {
      response: AsyncRefCell::new(response),
      stream: AsyncRefCell::new(stream),
    })
  };
  Ok(stream_rid)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeHttp2ClientResponse {
  headers: Vec<(ByteString, ByteString)>,
  body_rid: ResourceId,
}

#[op]
pub async fn op_node_http2_client_request_get_response(
  state: Rc<RefCell<OpState>>,
  stream_rid: ResourceId,
) -> Result<NodeHttp2ClientResponse, AnyError> {
  let resource = {
    let state = state.borrow();
    state
      .resource_table
      .get::<NodeHttp2ClientStream>(stream_rid)?
  };
  let mut response_future =
    RcRef::map(&resource, |r| &r.response).borrow_mut().await;

  let response = (&mut *response_future).await?;

  let (parts, body) = response.into_parts();
  let mut res_headers = Vec::new();
  for (key, val) in parts.headers.iter() {
    res_headers.push((key.as_str().into(), val.as_bytes().into()));
  }

  let body_rid = {
    let mut state = state.borrow_mut();
    state.resource_table.add(NodeHttp2ClientResponseBody {
      body: AsyncRefCell::new(body),
    })
  };
  Ok(NodeHttp2ClientResponse {
    headers: res_headers,
    body_rid,
  })
}

#[op]
pub async fn op_node_http2_client_request_get_response_body_chunk(
  state: Rc<RefCell<OpState>>,
  body_rid: ResourceId,
) -> Result<(Option<Vec<u8>>, bool), AnyError> {
  let resource = {
    let state = state.borrow();
    state
      .resource_table
      .get::<NodeHttp2ClientResponseBody>(body_rid)?
  };
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
    let mut state = state.borrow_mut();
    let _ = state.resource_table.close(body_rid);
    finished = true;
  }

  Ok((maybe_data, finished))
}
