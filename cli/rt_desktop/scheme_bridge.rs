// Copyright 2018-2026 the Deno authors. MIT license.

//! Bridges webview `app://` requests to the in-process `Deno.serve` server.
//!
//! The desktop runtime serves the user's `Deno.serve` app over an in-memory
//! byte channel (`DENO_SERVE_ADDRESS=memory:…`) instead of a plain TCP loopback.
//! The embedded browser can't speak to an in-memory channel directly, so we
//! register a laufey custom scheme handler for `app://`: each browser request
//! is delivered here, we open a fresh connection to the in-process listener,
//! speak HTTP/1.1 over it with a hyper client (the server side is hyper too),
//! and stream the response back to the webview.
//!
//! HTTP is fully handled that way. WebSockets can't be: modern webviews route
//! `ws://` through their own network stack (never through the scheme handler),
//! and even if they didn't, `laufey::SchemeExchange` is strictly HTTP one-shot
//! (`read_body` → `begin` → `write` → `finish`) — no upgrade/duplex primitive.
//!
//! To keep the memory transport for HTTP while unbreaking user code that does
//! `new WebSocket("ws://" + window.location.host)`, the desktop runtime binds a
//! narrow TCP loopback that only accepts WebSocket upgrades and proxies them
//! into the in-memory listener ([`proxy_ws_connection`] below). The webview
//! navigates to `app://127.0.0.1:PORT/` so `window.location.host` is
//! `127.0.0.1:PORT`, which `ws://` can then reach. Plain HTTP against the
//! loopback is rejected with 400 so the proxy is WebSocket-only — regular
//! requests still have to come through the `app://` scheme handler.

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
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

/// Name of the in-process memory listener the desktop app serves on. Shared
/// with the `DENO_SERVE_ADDRESS=memory:<name>` published at startup.
pub const DESKTOP_SERVE_NAME: &str = "deno-desktop";

/// Format the URL the webview should navigate to. The authority carries the
/// loopback port so `window.location.host` is `127.0.0.1:<port>`; when user
/// code does `new WebSocket("ws://" + window.location.host)` it lands on the
/// TCP loopback proxy below and gets bridged into the memory listener.
///
/// The scheme is still `app://` so HTTP requests continue to be intercepted
/// by the laufey scheme handler and served via the memory transport.
pub fn app_url(loopback_port: u16) -> String {
  format!("app://127.0.0.1:{}/", loopback_port)
}

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

// --- WebSocket loopback proxy ------------------------------------------------

/// Bind a WebSocket-only loopback proxy on `127.0.0.1:0` and start its accept
/// loop. Returns the port the OS chose so the caller can bake it into the
/// navigate URL.
///
/// The proxy is deliberately narrow: every accepted connection must present a
/// WebSocket handshake (`Upgrade: websocket` in the request headers) or it is
/// rejected with a 400 and closed. That keeps plain HTTP off the loopback so
/// the memory transport remains the only path in for regular requests.
pub async fn spawn_ws_loopback_proxy() -> Result<u16, std::io::Error> {
  let listener = TcpListener::bind("127.0.0.1:0").await?;
  let port = listener.local_addr()?.port();
  tokio::spawn(async move {
    loop {
      match listener.accept().await {
        Ok((stream, _peer)) => {
          tokio::spawn(async move {
            if let Err(e) = proxy_ws_connection(stream).await {
              log::debug!("[desktop] ws proxy connection error: {e}");
            }
          });
        }
        Err(e) => {
          log::error!("[desktop] ws proxy accept failed: {e}");
          // Backoff a beat so we don't tight-loop on a broken listener.
          tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
      }
    }
  });
  Ok(port)
}

async fn proxy_ws_connection(mut tcp: TcpStream) -> std::io::Result<()> {
  // Peek the client's request head. WebSocket handshakes are small; 8 KiB is
  // plenty for a real browser upgrade, and cheaper than fully parsing HTTP.
  let mut head = Vec::with_capacity(2048);
  let mut buf = [0u8; 2048];
  let end_of_head = loop {
    let n = tcp.read(&mut buf).await?;
    if n == 0 {
      return Ok(());
    }
    head.extend_from_slice(&buf[..n]);
    if let Some(idx) = find_end_of_head(&head) {
      break idx;
    }
    if head.len() >= 8 * 1024 {
      // Request head too large — bail without sending an HTTP response so we
      // don't leak that Deno.serve is behind us on plain-HTTP scans.
      return Ok(());
    }
  };

  if !is_websocket_upgrade(&head[..end_of_head]) {
    let _ = tcp
      .write_all(
        b"HTTP/1.1 400 Bad Request\r\n\
          content-type: text/plain; charset=utf-8\r\n\
          content-length: 51\r\n\
          connection: close\r\n\r\n\
          desktop loopback: WebSocket upgrade required\n",
      )
      .await;
    let _ = tcp.shutdown().await;
    return Ok(());
  }

  // Open the in-process connection to Deno.serve and replay everything we
  // already read from the client, then plain-shuttle bytes in both directions.
  let mut mem = match connect_memory(DESKTOP_SERVE_NAME) {
    Ok(s) => s,
    Err(e) => {
      log::warn!("[desktop] ws proxy: memory connect failed: {e}");
      return Ok(());
    }
  };
  mem.write_all(&head).await?;

  let _ = tokio::io::copy_bidirectional(&mut tcp, &mut mem).await;
  Ok(())
}

/// Byte offset of the request head/body boundary (`\r\n\r\n`) — the index one
/// past the final `\n`.
fn find_end_of_head(buf: &[u8]) -> Option<usize> {
  buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

/// Cheap header check: does the request head carry `Upgrade: websocket`? Case
/// is normalized because header names are ASCII-insensitive and browsers send
/// the value lowercase in practice but we don't want to depend on that.
fn is_websocket_upgrade(head: &[u8]) -> bool {
  // Skip past the request line to the first CRLF.
  let mut i = match head.windows(2).position(|w| w == b"\r\n") {
    Some(i) => i + 2,
    None => return false,
  };
  while i < head.len() {
    let line_end = match head[i..].windows(2).position(|w| w == b"\r\n") {
      Some(off) => i + off,
      None => head.len(),
    };
    let line = &head[i..line_end];
    if let Some(colon) = line.iter().position(|&b| b == b':') {
      let name = &line[..colon];
      let mut value = &line[colon + 1..];
      while let [b' ' | b'\t', rest @ ..] = value {
        value = rest;
      }
      if name.eq_ignore_ascii_case(b"upgrade") {
        // The Upgrade header can list multiple protocols; a browser only ever
        // sends `websocket`, but be forgiving of extra tokens/whitespace.
        for tok in value.split(|&b| b == b',') {
          let tok = trim_ascii(tok);
          if tok.eq_ignore_ascii_case(b"websocket") {
            return true;
          }
        }
      }
    }
    i = line_end + 2;
  }
  false
}

fn trim_ascii(mut b: &[u8]) -> &[u8] {
  while let [b' ' | b'\t', rest @ ..] = b {
    b = rest;
  }
  while let [rest @ .., b' ' | b'\t'] = b {
    b = rest;
  }
  b
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn path_and_query_preserves_path_query() {
    assert_eq!(path_and_query("app://desktop/foo?bar=baz"), "/foo?bar=baz");
    assert_eq!(path_and_query("app://desktop"), "/");
    // Authority carrying a port shouldn't leak into the request line.
    assert_eq!(path_and_query("app://127.0.0.1:5173/foo"), "/foo");
  }

  #[test]
  fn bridge_header_filtering() {
    assert!(should_skip_request_header("host"));
    assert!(should_skip_request_header("Connection"));
    assert!(is_hop_by_hop_header("transfer-encoding"));
    assert!(!should_skip_request_header("accept-language"));
    assert!(!is_hop_by_hop_header("content-type"));
  }

  #[test]
  fn app_url_format() {
    assert_eq!(app_url(50123), "app://127.0.0.1:50123/");
  }

  #[test]
  fn find_end_of_head_handles_split() {
    assert_eq!(find_end_of_head(b"GET / HTTP/1.1\r\n\r\n"), Some(18));
    assert_eq!(
      find_end_of_head(b"GET / HTTP/1.1\r\nHost: x\r\n\r\ntail"),
      Some(27),
    );
    assert_eq!(find_end_of_head(b"GET / HTTP/1.1\r\n"), None);
  }

  #[test]
  fn recognises_websocket_upgrade() {
    let ok = b"GET /chat HTTP/1.1\r\n\
      Host: 127.0.0.1:1234\r\n\
      Upgrade: websocket\r\n\
      Connection: Upgrade\r\n\
      Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
      Sec-WebSocket-Version: 13\r\n\r\n";
    assert!(is_websocket_upgrade(ok));

    // Case-insensitive header name + mixed-case value.
    let mixed = b"GET / HTTP/1.1\r\nUPGRADE: WebSocket\r\n\r\n";
    assert!(is_websocket_upgrade(mixed));

    // Extra tokens in the upgrade list should still match websocket.
    let list = b"GET / HTTP/1.1\r\nupgrade: h2c, websocket\r\n\r\n";
    assert!(is_websocket_upgrade(list));
  }

  #[test]
  fn rejects_non_upgrade_requests() {
    let plain = b"GET / HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n";
    assert!(!is_websocket_upgrade(plain));
    let other = b"POST /api HTTP/1.1\r\nUpgrade: h2c\r\n\r\n";
    assert!(!is_websocket_upgrade(other));
  }
}
