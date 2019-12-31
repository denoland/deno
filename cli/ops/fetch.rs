// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::io::StreamResource;
use crate::http_util::get_client;
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use bytes::Bytes;
use deno::*;
use futures::future::FutureExt;
use http::header::HeaderName;
use http::header::HeaderValue;
use http::Method;
use reqwest;
use reqwest::Response;
use std;
use std::cmp::min;
use std::convert::From;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncRead;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("fetch", s.core_op(json_op(s.stateful_op(op_fetch))));
}

#[derive(Deserialize)]
struct FetchArgs {
  method: Option<String>,
  url: String,
  headers: Vec<(String, String)>,
}

pub fn op_fetch(
  state: &ThreadSafeState,
  args: Value,
  data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: FetchArgs = serde_json::from_value(args)?;
  let url = args.url;

  let client = get_client();

  let method = match args.method {
    Some(method_str) => Method::from_bytes(method_str.as_bytes())?,
    None => Method::GET,
  };

  let url_ = url::Url::parse(&url).map_err(ErrBox::from)?;
  state.check_net_url(&url_)?;

  let mut request = client.request(method, url_);

  if let Some(buf) = data {
    request = request.body(Vec::from(&*buf));
  }

  for (key, value) in args.headers {
    let name = HeaderName::from_bytes(key.as_bytes()).unwrap();
    let v = HeaderValue::from_str(&value).unwrap();
    request = request.header(name, v);
  }
  debug!("Before fetch {}", url);
  let state_ = state.clone();

  let future = async move {
    let res = request.send().await?;
    debug!("Fetch response {}", url);
    let status = res.status();
    let mut res_headers = Vec::new();
    for (key, val) in res.headers().iter() {
      res_headers.push((key.to_string(), val.to_str().unwrap().to_owned()));
    }

    let body = HttpBody::from(res);
    let mut table = state_.lock_resource_table();
    let rid = table.add(
      "httpBody",
      Box::new(StreamResource::HttpBody(Box::new(body))),
    );

    let json_res = json!({
      "bodyRid": rid,
      "status": status.as_u16(),
      "statusText": status.canonical_reason().unwrap_or(""),
      "headers": res_headers
    });

    Ok(json_res)
  };

  Ok(JsonOp::Async(future.boxed()))
}

/// Wraps reqwest `Response` so that it can be exposed as an `AsyncRead` and integrated
/// into resources more easily.
pub struct HttpBody {
  response: Response,
  chunk: Option<Bytes>,
  pos: usize,
}

impl HttpBody {
  pub fn from(body: Response) -> Self {
    Self {
      response: body,
      chunk: None,
      pos: 0,
    }
  }
}

impl AsyncRead for HttpBody {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, io::Error>> {
    let mut inner = self.get_mut();
    if let Some(chunk) = inner.chunk.take() {
      debug!(
        "HttpBody Fake Read buf {} chunk {} pos {}",
        buf.len(),
        chunk.len(),
        inner.pos
      );
      let n = min(buf.len(), chunk.len() - inner.pos);
      {
        let rest = &chunk[inner.pos..];
        buf[..n].clone_from_slice(&rest[..n]);
      }
      inner.pos += n;
      if inner.pos == chunk.len() {
        inner.pos = 0;
      } else {
        inner.chunk = Some(chunk);
      }
      return Poll::Ready(Ok(n));
    } else {
      assert_eq!(inner.pos, 0);
    }

    let chunk_future = &mut inner.response.chunk();
    // Safety: `chunk_future` lives only for duration of this poll. So, it doesn't move.
    let chunk_future = unsafe { Pin::new_unchecked(chunk_future) };
    match chunk_future.poll(cx) {
      Poll::Ready(Err(e)) => {
        Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
      }
      Poll::Ready(Ok(Some(chunk))) => {
        debug!(
          "HttpBody Real Read buf {} chunk {} pos {}",
          buf.len(),
          chunk.len(),
          inner.pos
        );
        let n = min(buf.len(), chunk.len());
        buf[..n].clone_from_slice(&chunk[..n]);
        if buf.len() < chunk.len() {
          inner.pos = n;
          inner.chunk = Some(chunk);
        }
        Poll::Ready(Ok(n))
      }
      Poll::Ready(Ok(None)) => Poll::Ready(Ok(0)),
      Poll::Pending => Poll::Pending,
    }
  }
}
