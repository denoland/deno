// Copyright 2018-2026 the Deno authors. MIT license.

//! Bridges webview `app://` requests to the in-process Deno.serve server.
//!
//! The desktop runtime serves the user's `Deno.serve` app over an in-memory
//! byte channel (`DENO_SERVE_ADDRESS=memory:…`) instead of a TCP loopback. The
//! embedded browser can't speak to an in-memory channel directly, so we
//! register a laufey custom scheme handler for `app://`: each browser request
//! is delivered here, we open a fresh connection to the in-process listener,
//! speak HTTP/1.1 over it with a hyper client (the server side is hyper too),
//! and stream the response back to the webview.

use deno_net::memory::connect_memory;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::header::CONNECTION;
use hyper::header::HOST;
use hyper::header::PROXY_AUTHENTICATE;
use hyper::header::PROXY_AUTHORIZATION;
use hyper::header::TE;
use hyper::header::TRAILER;
use hyper::header::TRANSFER_ENCODING;
use hyper::header::UPGRADE;
use hyper_util::rt::TokioIo;
use laufey::SchemeRequest;

/// Name of the in-process memory listener the desktop app serves on. Shared
/// with the `DENO_SERVE_ADDRESS=memory:<name>` published at startup.
pub const DESKTOP_SERVE_NAME: &str = "deno-desktop";

/// The URL the webview navigates to. The authority is cosmetic — every request
/// is bridged into [`DESKTOP_SERVE_NAME`] regardless of host.
pub const APP_URL: &str = "app://desktop/";

type BridgeError = Box<dyn std::error::Error + Send + Sync>;

/// Register the `app://` scheme handler on the current tokio runtime. Each
/// request is bridged on its own spawned task so the laufey IO thread is never
/// blocked. Must be called from within the Deno tokio runtime context.
pub fn register() {
  let handle = tokio::runtime::Handle::current();
  laufey::register_scheme_handler("app", move |req| {
    handle.spawn(handle_request(req));
  });
}

async fn handle_request(req: SchemeRequest) {
  let exchange = req.exchange;
  let mut began = false;
  if let Err(e) = Box::pin(bridge(
    &req.method,
    &req.url,
    &req.headers,
    &exchange,
    &mut began,
  ))
  .await
  {
    log::error!("[desktop] app:// bridge error: {e}");
    if !began {
      // Surface a minimal error page if we never sent a response head.
      exchange.begin(
        502,
        &[(
          "content-type".to_string(),
          "text/plain; charset=utf-8".to_string(),
        )],
      );
      let _ =
        exchange.write(format!("desktop transport error: {e}").as_bytes());
    }
  }
  exchange.finish();
}

async fn bridge(
  method: &str,
  url: &str,
  headers: &[(String, String)],
  exchange: &laufey::SchemeExchange,
  began: &mut bool,
) -> Result<(), BridgeError> {
  // The request body is fully buffered by the backend, so these pulls are
  // non-blocking copies.
  let mut body = Vec::new();
  let mut buf = [0u8; 16 * 1024];
  loop {
    let n = exchange.read_body(&mut buf);
    if n <= 0 {
      break;
    }
    body.extend_from_slice(&buf[..n as usize]);
  }

  // Open a fresh in-process connection to the Deno.serve listener and drive an
  // HTTP/1.1 client over it.
  let stream = connect_memory(DESKTOP_SERVE_NAME)?;
  let io = TokioIo::new(stream);
  let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
  tokio::spawn(async move {
    let _ = conn.await;
  });

  let mut builder = hyper::Request::builder()
    .method(method)
    .uri(path_and_query(url));
  for (name, value) in headers {
    if should_skip_request_header(name) {
      continue;
    }
    builder = builder.header(name.as_str(), value.as_str());
  }
  builder = builder.header(HOST, "desktop");
  let request = builder.body(Full::new(bytes::Bytes::from(body)))?;

  let response = sender.send_request(request).await?;

  let status = response.status().as_u16() as i32;
  let mut resp_headers = Vec::with_capacity(response.headers().len());
  for (name, value) in response.headers() {
    if is_hop_by_hop_header(name.as_str()) {
      continue;
    }
    if let Ok(v) = value.to_str() {
      resp_headers.push((name.as_str().to_string(), v.to_string()));
    }
  }
  exchange.begin(status, &resp_headers);
  *began = true;

  let mut body = response.into_body();
  while let Some(frame) = body.frame().await {
    let frame = frame?;
    if let Some(chunk) = frame.data_ref() {
      // Negative return means the webview cancelled / went away.
      if exchange.write(chunk.as_ref()) < 0 {
        break;
      }
    }
  }

  Ok(())
}

/// Extract the path-and-query from an `app://authority/path?query` URL, the
/// form a hyper request target needs.
fn path_and_query(url: &str) -> String {
  match url.split_once("://") {
    Some((_, rest)) => match rest.find('/') {
      Some(i) => rest[i..].to_string(),
      None => "/".to_string(),
    },
    None => url.to_string(),
  }
}

fn should_skip_request_header(name: &str) -> bool {
  name.eq_ignore_ascii_case(HOST.as_str()) || is_hop_by_hop_header(name)
}

fn is_hop_by_hop_header(name: &str) -> bool {
  name.eq_ignore_ascii_case(CONNECTION.as_str())
    || name.eq_ignore_ascii_case("keep-alive")
    || name.eq_ignore_ascii_case(PROXY_AUTHENTICATE.as_str())
    || name.eq_ignore_ascii_case(PROXY_AUTHORIZATION.as_str())
    || name.eq_ignore_ascii_case(TE.as_str())
    || name.eq_ignore_ascii_case(TRAILER.as_str())
    || name.eq_ignore_ascii_case(TRANSFER_ENCODING.as_str())
    || name.eq_ignore_ascii_case(UPGRADE.as_str())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn path_and_query_preserves_path_query() {
    assert_eq!(path_and_query("app://desktop/foo?bar=baz"), "/foo?bar=baz");
    assert_eq!(path_and_query("app://desktop"), "/");
  }

  #[test]
  fn bridge_header_filtering() {
    assert!(should_skip_request_header("host"));
    assert!(should_skip_request_header("Connection"));
    assert!(is_hop_by_hop_header("transfer-encoding"));
    assert!(!should_skip_request_header("accept-language"));
    assert!(!is_hop_by_hop_header("content-type"));
  }
}
