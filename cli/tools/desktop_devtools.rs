// Copyright 2018-2026 the Deno authors. MIT license.

//! CDP multiplexer for `deno desktop --inspect`.
//!
//! Architecture:
//!
//! ```text
//!                 ┌──────────────────────────────────┐
//!                 │   CDP Multiplexer (this file)    │
//!   DevTools ◄──► │   - /json/version, /json/list    │
//!   (single ws)   │   - /unified  (primary entry)    │
//!                 │   - /deno, /cef (direct bypass)  │
//!                 │   - /devtools/* (frontend proxy) │
//!                 └──────┬───────────────────┬───────┘
//!                        │                   │
//!              Deno inspector          CEF renderer
//!              (internal port)         (internal port)
//! ```
//!
//! The `/unified` endpoint is the primary session. CEF is the default
//! (un-sessioned) target; the Deno runtime appears as an attached child
//! via a synthetic `Target.attachedToTarget` event with a stable
//! `sessionId`. The mux routes frames by `sessionId` — Deno-bound
//! frames have the sessionId stripped before forwarding, and responses
//! get it re-injected. Everything else goes to CEF verbatim.
//!
//! DevTools sees both isolates in one window: the Console dropdown
//! shows "Renderer" / "Deno", and the Sources panel Threads sidebar
//! lists both.
//!
//! `/deno` and `/cef` are direct passthrough endpoints for debugging
//! each isolate in isolation. `/devtools/*` proxies CEF's bundled
//! DevTools frontend assets so `openDevtools()` can pop a CEF window
//! without triggering the remote-debugging-port interception.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use deno_core::anyhow::Context;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use fastwebsockets::Frame;
use fastwebsockets::OpCode;
use fastwebsockets::WebSocket;
use fastwebsockets::WebSocketError;
use fastwebsockets::handshake;
use http_body_util::BodyExt;
use http_body_util::Empty;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use uuid::Uuid;

/// Configuration for the CDP multiplexer.
#[derive(Clone, Debug)]
pub struct MuxConfig {
  /// User-visible listen address (from `--inspect`).
  pub listen: SocketAddr,
  /// Internal listen address for Deno's native inspector.
  pub deno_internal: SocketAddr,
  /// Internal listen address for the CEF renderer debug port.
  pub cef_internal: SocketAddr,
  /// `true` when `--inspect-brk` was passed — the mux will inject
  /// `Debugger.enable` + `Debugger.pause` into the CEF session so the
  /// renderer breaks on the first JS statement after navigation.
  pub inspect_brk: bool,
  /// `true` when `--inspect-wait` or `--inspect-brk` was passed.
  /// Not read by the mux itself — the child process uses env vars to
  /// decide whether to poll `/debugger-attached` before navigating.
  #[allow(dead_code)]
  pub wait_for_debugger: bool,
}

/// A running multiplexer. Dropping the handle shuts the server down.
pub struct MuxHandle {
  pub listen: SocketAddr,
  _shutdown_tx: oneshot::Sender<()>,
}

/// Bind to an ephemeral TCP port and return its number. The listener is
/// dropped before the port is reused by the caller; on the loopback
/// interface the small race window is acceptable for a debugger-only port.
pub fn allocate_random_port() -> std::io::Result<u16> {
  let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
  Ok(listener.local_addr()?.port())
}

/// Spawn the mux on a background task. Returns once the listener is
/// bound — connection handling continues in the spawned task. The
/// upstream servers (Deno inspector, CEF) do not need to be up yet;
/// the mux polls them on demand when DevTools makes a request.
pub async fn spawn_mux(config: MuxConfig) -> Result<MuxHandle, AnyError> {
  let listener = TcpListener::bind(config.listen).await.with_context(|| {
    format!("failed to bind CDP multiplexer to {}", config.listen)
  })?;
  let listen = listener.local_addr()?;
  let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

  let state = Arc::new(MuxState::new(config.clone(), listen));

  tokio::spawn(async move {
    let mut shutdown_rx = shutdown_rx;
    loop {
      tokio::select! {
        _ = &mut shutdown_rx => {
          log::debug!("[devtools-mux] shutdown requested");
          break;
        }
        accept = listener.accept() => {
          match accept {
            Ok((stream, _)) => {
              let state = state.clone();
              tokio::spawn(async move {
                if let Err(err) = serve_connection(stream, state).await {
                  log::debug!("[devtools-mux] connection error: {err:?}");
                }
              });
            }
            Err(err) => {
              log::error!("[devtools-mux] accept failed: {err:?}");
              tokio::time::sleep(Duration::from_millis(200)).await;
            }
          }
        }
      }
    }
  });

  Ok(MuxHandle {
    listen,
    _shutdown_tx: shutdown_tx,
  })
}

/// Per-target identification. Kept small and stable so DevTools'
/// `webSocketDebuggerUrl` doesn't change across polls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TargetKind {
  /// Single CDP session fronting both isolates. CEF is the primary;
  /// Deno appears as an attached child target via `Target.*`.
  Unified,
  /// Direct passthrough to the Deno inspector.
  Deno,
  /// Direct passthrough to the CEF renderer's debug port.
  Cef,
}

impl TargetKind {
  fn path(self) -> &'static str {
    match self {
      TargetKind::Unified => "/unified",
      TargetKind::Deno => "/deno",
      TargetKind::Cef => "/cef",
    }
  }

  fn title(self) -> &'static str {
    match self {
      TargetKind::Unified => "Deno Desktop (unified)",
      TargetKind::Deno => "Deno Runtime",
      TargetKind::Cef => "CEF Renderer",
    }
  }
}

/// Synthetic CDP target id for the Deno isolate inside the unified
/// session. Stable per process; DevTools uses it in `Target.attachToTarget`.
const DENO_CHILD_TARGET_ID: &str = "deno-runtime-isolate";

struct MuxState {
  config: MuxConfig,
  listen: SocketAddr,
  // Stable UUIDs per target so repeated /json/list calls return the
  // same IDs. DevTools caches these.
  unified_id: Uuid,
  deno_id: Uuid,
  cef_id: Uuid,
  // Stable session id we hand DevTools when it attaches to the Deno
  // child target inside the unified session.
  deno_session_id: String,
  // Set to `true` when a DevTools client has connected to any session.
  // The child process polls `/debugger-attached` to gate navigation
  // when `--inspect-wait` or `--inspect-brk` is active.
  debugger_attached: Arc<std::sync::atomic::AtomicBool>,
}

impl MuxState {
  fn new(config: MuxConfig, listen: SocketAddr) -> Self {
    Self {
      config,
      listen,
      unified_id: Uuid::new_v4(),
      deno_id: Uuid::new_v4(),
      cef_id: Uuid::new_v4(),
      deno_session_id: Uuid::new_v4().to_string(),
      debugger_attached: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    }
  }

  fn target_for_path(&self, path: &str) -> Option<TargetKind> {
    if path == TargetKind::Unified.path() {
      Some(TargetKind::Unified)
    } else if path == TargetKind::Deno.path() {
      Some(TargetKind::Deno)
    } else if path == TargetKind::Cef.path() {
      Some(TargetKind::Cef)
    } else {
      None
    }
  }
}

async fn serve_connection(
  stream: TcpStream,
  state: Arc<MuxState>,
) -> Result<(), AnyError> {
  let io = TokioIo::new(stream);
  let service = hyper::service::service_fn(move |req| {
    let state = state.clone();
    async move { Ok::<_, Infallible>(handle_request(req, state).await) }
  });

  hyper::server::conn::http1::Builder::new()
    .serve_connection(io, service)
    .with_upgrades()
    .await
    .map_err(|e| anyhow!("hyper serve error: {e}"))?;
  Ok(())
}

async fn handle_request(
  req: hyper::Request<Incoming>,
  state: Arc<MuxState>,
) -> hyper::Response<Full<Bytes>> {
  if req.method() != http::Method::GET {
    return simple_response(
      http::StatusCode::METHOD_NOT_ALLOWED,
      "Not Allowed",
    );
  }
  let path = req.uri().path().to_string();
  match path.as_str() {
    "/json/version" => json_version(&state),
    "/json" | "/json/list" => json_list(&state).await,
    "/json/protocol" => json_protocol(),
    "/debugger-attached" => {
      if state
        .debugger_attached
        .load(std::sync::atomic::Ordering::SeqCst)
      {
        simple_response(http::StatusCode::OK, "attached")
      } else {
        simple_response(http::StatusCode::SERVICE_UNAVAILABLE, "waiting")
      }
    }
    other => {
      if let Some(kind) = state.target_for_path(other) {
        match handle_upgrade(req, kind, state.clone()).await {
          Ok(resp) => resp,
          Err(err) => {
            log::error!("[devtools-mux] upgrade failed for {other}: {err:?}");
            simple_response(http::StatusCode::BAD_REQUEST, "upgrade failed")
          }
        }
      } else if other.starts_with("/devtools/") {
        // Proxy DevTools frontend assets from CEF's bundled HTTP server.
        // Serving them through our port means the CEF window navigating
        // to the DevTools URL sees only `127.0.0.1:<mux_port>`, so it
        // doesn't special-case the remote-debugging port and steal the
        // frontend for the new window's own renderer.
        match proxy_devtools_asset(&req, state.config.cef_internal).await {
          Ok(resp) => resp,
          Err(err) => {
            log::error!("[devtools-mux] devtools asset proxy failed: {err:?}");
            simple_response(http::StatusCode::BAD_GATEWAY, "proxy failed")
          }
        }
      } else {
        simple_response(http::StatusCode::NOT_FOUND, "Not Found")
      }
    }
  }
}

/// GET `http://<cef_internal><path>` and return the response verbatim.
/// Used to proxy the bundled DevTools frontend (inspector.html + its
/// JS/CSS assets) through the mux's own port, bypassing CEF's
/// remote-debugging-port special-case that would otherwise wire the
/// frontend to the requesting window's renderer.
async fn proxy_devtools_asset(
  req: &hyper::Request<Incoming>,
  cef_internal: SocketAddr,
) -> Result<hyper::Response<Full<Bytes>>, AnyError> {
  let path_and_query = req
    .uri()
    .path_and_query()
    .map(|p| p.as_str())
    .unwrap_or("/");

  let stream = TcpStream::connect(cef_internal).await?;
  let io = TokioIo::new(stream);
  let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
    .await
    .map_err(|e| anyhow!("devtools asset handshake failed: {e}"))?;
  tokio::spawn(async move {
    if let Err(err) = conn.await {
      log::trace!("[devtools-mux] devtools asset conn closed: {err:?}");
    }
  });

  let upstream_req = hyper::Request::builder()
    .method(http::Method::GET)
    .uri(path_and_query)
    .header(http::header::HOST, cef_internal.to_string())
    .body(Empty::<Bytes>::new())?;
  let resp = sender.send_request(upstream_req).await?;
  let (parts, body) = resp.into_parts();
  let bytes = body.collect().await?.to_bytes();

  let mut builder = hyper::Response::builder().status(parts.status);
  // Drop hop-by-hop headers that don't apply to our re-packaged body.
  for (name, value) in parts.headers.iter() {
    let skip = matches!(
      name.as_str().to_ascii_lowercase().as_str(),
      "transfer-encoding"
        | "content-length"
        | "connection"
        | "keep-alive"
        | "proxy-authenticate"
        | "proxy-authorization"
        | "te"
        | "trailer"
        | "upgrade"
    );
    if !skip {
      builder = builder.header(name, value);
    }
  }
  Ok(
    builder
      .header(http::header::CONTENT_LENGTH, bytes.len())
      .body(Full::new(bytes))
      .unwrap(),
  )
}

fn json_version(state: &MuxState) -> hyper::Response<Full<Bytes>> {
  let body = json!({
    "Browser": format!("deno-desktop/{}", env!("CARGO_PKG_VERSION")),
    "Protocol-Version": "1.3",
    "V8-Version": deno_core::v8::VERSION_STRING,
    // Advertise one of the two upstream WS URLs as the "browser" URL.
    // DevTools' `chrome://inspect` uses this to drive auto-attach; the
    // CEF upstream exposes the richer Target.* domain.
    "webSocketDebuggerUrl": format!(
      "ws://{}{}",
      state.listen,
      TargetKind::Cef.path(),
    ),
  });
  json_response(body)
}

async fn json_list(state: &MuxState) -> hyper::Response<Full<Bytes>> {
  let listen = state.listen.to_string();
  let unified_url = format!("ws://{listen}{}", TargetKind::Unified.path());
  let deno_url = format!("ws://{listen}{}", TargetKind::Deno.path());
  let cef_url = format!("ws://{listen}{}", TargetKind::Cef.path());

  // Primary entry: the unified session. DevTools opens one window
  // (inspector.html — full browser DevTools) and gets both isolates as
  // attached targets in the Sources panel.
  let unified_entry = json!({
    "id": state.unified_id.to_string(),
    "type": "page",
    "title": TargetKind::Unified.title(),
    "description": "Unified DevTools (CEF page + Deno runtime)",
    "url": "deno-desktop://unified",
    "faviconUrl": "https://deno.land/favicon.ico",
    "devtoolsFrontendUrl": format!(
      "devtools://devtools/bundled/inspector.html?ws={}",
      strip_scheme(&unified_url),
    ),
    "webSocketDebuggerUrl": unified_url,
  });
  // Fallback entries: direct passthrough to each isolate. Useful when
  // the unified session misbehaves and you want to debug each side in
  // isolation.
  let deno_entry = json!({
    "id": state.deno_id.to_string(),
    "type": "node",
    "title": TargetKind::Deno.title(),
    "description": "Deno runtime V8 isolate (direct)",
    "url": format!("deno://{}", state.config.deno_internal),
    "faviconUrl": "https://deno.land/favicon.ico",
    "devtoolsFrontendUrl": format!(
      "devtools://devtools/bundled/js_app.html?ws={}&experiments=true&v8only=true",
      strip_scheme(&deno_url),
    ),
    "webSocketDebuggerUrl": deno_url,
  });
  let cef_entry = json!({
    "id": state.cef_id.to_string(),
    "type": "page",
    "title": TargetKind::Cef.title(),
    "description": "CEF renderer V8 isolate (direct)",
    "url": format!("cef://{}", state.config.cef_internal),
    "faviconUrl": "https://deno.land/favicon.ico",
    "devtoolsFrontendUrl": format!(
      "devtools://devtools/bundled/inspector.html?ws={}",
      strip_scheme(&cef_url),
    ),
    "webSocketDebuggerUrl": cef_url,
  });

  json_response(Value::Array(vec![unified_entry, deno_entry, cef_entry]))
}

/// Return an empty protocol descriptor. DevTools tolerates this and
/// falls back to the built-in protocol.
fn json_protocol() -> hyper::Response<Full<Bytes>> {
  json_response(json!({
    "version": { "major": "1", "minor": "3" },
    "domains": [],
  }))
}

fn json_response(value: Value) -> hyper::Response<Full<Bytes>> {
  let body = Full::new(Bytes::from(serde_json::to_vec(&value).unwrap()));
  hyper::Response::builder()
    .status(http::StatusCode::OK)
    .header(http::header::CONTENT_TYPE, "application/json")
    .body(body)
    .unwrap()
}

fn simple_response(
  status: http::StatusCode,
  msg: &'static str,
) -> hyper::Response<Full<Bytes>> {
  hyper::Response::builder()
    .status(status)
    .body(Full::new(Bytes::from(msg)))
    .unwrap()
}

fn strip_scheme(ws_url: &str) -> String {
  ws_url
    .strip_prefix("ws://")
    .or_else(|| ws_url.strip_prefix("wss://"))
    .unwrap_or(ws_url)
    .to_string()
}

/// Upgrade an incoming HTTP request to a WebSocket, open a matching
/// WebSocket to the upstream target, and shuttle frames in both
/// directions.
async fn handle_upgrade(
  mut req: hyper::Request<Incoming>,
  kind: TargetKind,
  state: Arc<MuxState>,
) -> Result<hyper::Response<Full<Bytes>>, AnyError> {
  let (resp, upgrade_fut) = fastwebsockets::upgrade::upgrade(&mut req)
    .map_err(|e| anyhow!("not a valid websocket upgrade: {e}"))?;

  tokio::spawn(async move {
    let client = match upgrade_fut.await {
      Ok(ws) => ws,
      Err(err) => {
        log::error!("[devtools-mux] client upgrade failed: {err:?}");
        return;
      }
    };

    // Signal that a debugger has connected — the child process polls
    // `/debugger-attached` to gate navigation under --inspect-wait/brk.
    state
      .debugger_attached
      .store(true, std::sync::atomic::Ordering::SeqCst);

    match kind {
      TargetKind::Unified => {
        if let Err(err) = run_unified_session(client, state).await {
          log::debug!("[devtools-mux] unified session ended: {err:?}");
        }
      }
      TargetKind::Deno | TargetKind::Cef => {
        match connect_upstream(&state, kind).await {
          Ok(upstream) => {
            if let Err(err) = proxy_frames(client, upstream).await {
              log::debug!("[devtools-mux] proxy ended: {err:?}");
            }
          }
          Err(err) => {
            log::error!(
              "[devtools-mux] failed to connect upstream for {kind:?}: {err:?}"
            );
          }
        }
      }
    }
  });

  let (parts, _) = resp.into_parts();
  Ok(hyper::Response::from_parts(parts, Full::new(Bytes::new())))
}

/// Open a WebSocket to the right upstream target, performing target
/// discovery as needed. For CEF we hit `/json/list` to find the live
/// `webSocketDebuggerUrl`; for Deno we call the inspector server's
/// same endpoint. The upstream is retried briefly since the renderer
/// may not have finished booting.
async fn connect_upstream(
  state: &MuxState,
  kind: TargetKind,
) -> Result<WebSocket<TokioIo<hyper::upgrade::Upgraded>>, AnyError> {
  let upstream_host = match kind {
    TargetKind::Deno => state.config.deno_internal,
    TargetKind::Cef => state.config.cef_internal,
    TargetKind::Unified => {
      bail!("connect_upstream cannot be called with the Unified target")
    }
  };

  // Poll until the upstream has a live WebSocket debugger URL.
  let mut last_err: Option<AnyError> = None;
  let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
  while tokio::time::Instant::now() < deadline {
    match fetch_upstream_ws_url(upstream_host).await {
      Ok(ws_url) => match connect_ws(&ws_url).await {
        Ok(ws) => return Ok(ws),
        Err(err) => {
          last_err = Some(err);
        }
      },
      Err(err) => {
        last_err = Some(err);
      }
    }
    tokio::time::sleep(Duration::from_millis(250)).await;
  }
  Err(last_err.unwrap_or_else(|| anyhow!("upstream connect timed out")))
}

/// GET `http://<host>/json/list` and pick the first entry's
/// `webSocketDebuggerUrl`. Both Deno's inspector server and CEF's
/// remote-debugging endpoint implement this.
async fn fetch_upstream_ws_url(host: SocketAddr) -> Result<String, AnyError> {
  let stream = TcpStream::connect(host).await?;
  let io = TokioIo::new(stream);
  let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
    .await
    .map_err(|e| anyhow!("http handshake to {host} failed: {e}"))?;
  tokio::spawn(async move {
    if let Err(err) = conn.await {
      log::trace!("[devtools-mux] upstream conn closed: {err:?}");
    }
  });

  let req = hyper::Request::builder()
    .method(http::Method::GET)
    .uri("/json/list")
    .header(http::header::HOST, host.to_string())
    .body(Empty::<Bytes>::new())?;
  let resp = sender.send_request(req).await?;
  if !resp.status().is_success() {
    bail!("upstream /json/list at {host} returned {}", resp.status());
  }
  let body = resp.collect().await?.to_bytes();
  let value: Value = serde_json::from_slice(&body)
    .with_context(|| format!("upstream /json/list at {host} not JSON"))?;

  let ws_url = value
    .as_array()
    .and_then(|arr| {
      arr.iter().find_map(|v| {
        // Skip targets that are our own DevTools frontend window — when
        // openDevtools() creates a CEF window pointed at inspector.html,
        // CEF registers it as a debuggable target. Connecting to it
        // instead of the real app window would show "DevTools for
        // DevTools".
        let url = v.get("url").and_then(|u| u.as_str()).unwrap_or("");
        if url.contains("/devtools/") || url.contains("devtools://") {
          return None;
        }
        v.get("webSocketDebuggerUrl")
      })
    })
    .and_then(|v| v.as_str())
    .ok_or_else(|| {
      anyhow!("no webSocketDebuggerUrl in /json/list at {host}")
    })?;

  // Upstream responds with its own listen host; some backends return
  // `0.0.0.0` or `localhost`. Force to the target host so we connect
  // to the right address.
  let rewritten = rewrite_ws_host(ws_url, host);
  Ok(rewritten)
}

fn rewrite_ws_host(ws_url: &str, host: SocketAddr) -> String {
  let rest = ws_url
    .strip_prefix("ws://")
    .or_else(|| ws_url.strip_prefix("wss://"))
    .unwrap_or(ws_url);
  let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("/");
  format!("ws://{host}{path}")
}

async fn connect_ws(
  ws_url: &str,
) -> Result<WebSocket<TokioIo<hyper::upgrade::Upgraded>>, AnyError> {
  let url: http::Uri = ws_url.parse()?;
  let host = url
    .host()
    .ok_or_else(|| anyhow!("ws url missing host: {ws_url}"))?;
  let port = url.port_u16().unwrap_or(80);
  let authority = format!("{host}:{port}");

  let stream = TcpStream::connect(&authority).await?;
  let req = hyper::Request::builder()
    .method(http::Method::GET)
    .uri(url.path_and_query().map(|p| p.as_str()).unwrap_or("/"))
    .header(http::header::HOST, &authority)
    .header(http::header::UPGRADE, "websocket")
    .header(http::header::CONNECTION, "upgrade")
    .header("Sec-WebSocket-Key", handshake::generate_key())
    .header("Sec-WebSocket-Version", "13")
    .body(Empty::<Bytes>::new())?;

  let (ws, _) = handshake::client(&TokioExec, req, stream).await?;
  Ok(ws)
}

struct TokioExec;
impl<F> hyper::rt::Executor<F> for TokioExec
where
  F: std::future::Future + Send + 'static,
  F::Output: Send + 'static,
{
  fn execute(&self, fut: F) {
    tokio::spawn(fut);
  }
}

/// Bidirectionally forward frames between the DevTools client and the
/// upstream inspector. The loop ends when either side closes or errors.
async fn proxy_frames(
  mut client: WebSocket<TokioIo<hyper::upgrade::Upgraded>>,
  mut upstream: WebSocket<TokioIo<hyper::upgrade::Upgraded>>,
) -> Result<(), AnyError> {
  // We forward control frames (ping/pong/close) verbatim between the
  // two peers, so disable fastwebsockets' built-in handling.
  client.set_auto_close(false);
  client.set_auto_pong(false);
  upstream.set_auto_close(false);
  upstream.set_auto_pong(false);

  // Split both sides so the two pump directions can run concurrently
  // without holding a single mutex across `.await` points.
  let (mut client_rx, mut client_tx) = client.split(tokio::io::split);
  let (mut up_rx, mut up_tx) = upstream.split(tokio::io::split);

  let client_to_up = async {
    // The send_fn is used by fastwebsockets to auto-respond to control
    // frames; with auto_close/auto_pong disabled it is never called.
    let mut noop = |_: Frame<'_>| async { Ok::<(), WebSocketError>(()) };
    loop {
      let frame = match client_rx.read_frame(&mut noop).await {
        Ok(f) => f,
        Err(err) => {
          log::debug!("[devtools-mux] client read: {err:?}");
          return;
        }
      };
      let is_close = frame.opcode == OpCode::Close;
      if let Err(err) = up_tx.write_frame(frame).await {
        log::debug!("[devtools-mux] upstream write: {err:?}");
        return;
      }
      if is_close {
        return;
      }
    }
  };

  let up_to_client = async {
    let mut noop = |_: Frame<'_>| async { Ok::<(), WebSocketError>(()) };
    loop {
      let frame = match up_rx.read_frame(&mut noop).await {
        Ok(f) => f,
        Err(err) => {
          log::debug!("[devtools-mux] upstream read: {err:?}");
          return;
        }
      };
      let is_close = frame.opcode == OpCode::Close;
      if let Err(err) = client_tx.write_frame(frame).await {
        log::debug!("[devtools-mux] client write: {err:?}");
        return;
      }
      if is_close {
        return;
      }
    }
  };

  tokio::join!(client_to_up, up_to_client);
  Ok(())
}

// ─── Unified session (v2) ──────────────────────────────────────────
//
// One DevTools window, two isolates. CEF is the primary session;
// Deno appears as an "attached" child target via the standard
// `Target.attachedToTarget` event with a synthetic `sessionId`.
// Frames flow through CDP-aware routers that strip/inject `sessionId`
// on the Deno leg and pass everything else through to CEF.

/// Owned representation of a WebSocket frame, suitable for sending
/// through an mpsc channel. `fastwebsockets::Frame` borrows from the
/// reader's internal buffer and so can't cross task boundaries.
struct OwnedFrame {
  opcode: OpCode,
  payload: Vec<u8>,
}

impl OwnedFrame {
  fn text(payload: Vec<u8>) -> Self {
    Self {
      opcode: OpCode::Text,
      payload,
    }
  }

  fn into_frame(self) -> Frame<'static> {
    Frame::new(
      true,
      self.opcode,
      None,
      fastwebsockets::Payload::Owned(self.payload),
    )
  }
}

/// Run the unified DevTools session: one client WebSocket fronting two
/// upstreams (CEF as the default session, Deno attached via a
/// synthetic `sessionId`).
async fn run_unified_session(
  client: WebSocket<TokioIo<hyper::upgrade::Upgraded>>,
  state: Arc<MuxState>,
) -> Result<(), AnyError> {
  let mut client = client;
  client.set_auto_close(false);
  client.set_auto_pong(false);

  let (cef, deno) = tokio::try_join!(
    connect_upstream(&state, TargetKind::Cef),
    connect_upstream(&state, TargetKind::Deno),
  )?;
  let mut cef = cef;
  let mut deno = deno;
  cef.set_auto_close(false);
  cef.set_auto_pong(false);
  deno.set_auto_close(false);
  deno.set_auto_pong(false);

  let session_id = state.deno_session_id.clone();

  // When --inspect-brk is active, inject Debugger.enable + Debugger.pause
  // into the CEF session BEFORE forwarding any client frames. This ensures
  // the renderer pauses on the very first JS statement after navigation.
  if state.config.inspect_brk {
    let enable = json!({"id": -1, "method": "Debugger.enable"});
    let enable_bytes = serde_json::to_vec(&enable).unwrap();
    cef
      .write_frame(Frame::new(
        true,
        OpCode::Text,
        None,
        fastwebsockets::Payload::Owned(enable_bytes),
      ))
      .await?;

    // Read the enable response before sending pause.
    let _resp = cef.read_frame().await?;

    let pause = json!({"id": -2, "method": "Debugger.pause"});
    let pause_bytes = serde_json::to_vec(&pause).unwrap();
    cef
      .write_frame(Frame::new(
        true,
        OpCode::Text,
        None,
        fastwebsockets::Payload::Owned(pause_bytes),
      ))
      .await?;
    let _resp = cef.read_frame().await?;

    log::debug!(
      "[devtools-mux] injected Debugger.enable + Debugger.pause into CEF"
    );
  }

  let (mut client_rx, mut client_tx) = client.split(tokio::io::split);
  let (mut cef_rx, mut cef_tx) = cef.split(tokio::io::split);
  let (mut deno_rx, mut deno_tx) = deno.split(tokio::io::split);

  let (client_send, mut client_recv) = mpsc::unbounded_channel::<OwnedFrame>();
  let (cef_send, mut cef_recv) = mpsc::unbounded_channel::<OwnedFrame>();
  let (deno_send, mut deno_recv) = mpsc::unbounded_channel::<OwnedFrame>();

  // Deno is announced lazily, the first time DevTools asks about
  // targets — firing too early causes the event to be dropped before
  // the frontend's auto-attach manager has subscribed. See
  // `route_client_text` for the trigger.
  let deno_announced = Arc::new(std::sync::atomic::AtomicBool::new(false));

  // Writer tasks: pump owned frames from a channel into the WS half.
  let client_writer = tokio::spawn(async move {
    while let Some(owned) = client_recv.recv().await {
      let close = owned.opcode == OpCode::Close;
      if let Err(err) = client_tx.write_frame(owned.into_frame()).await {
        log::debug!("[devtools-mux] unified client write: {err:?}");
        return;
      }
      if close {
        return;
      }
    }
  });
  let cef_writer = tokio::spawn(async move {
    while let Some(owned) = cef_recv.recv().await {
      let close = owned.opcode == OpCode::Close;
      if let Err(err) = cef_tx.write_frame(owned.into_frame()).await {
        log::debug!("[devtools-mux] unified cef write: {err:?}");
        return;
      }
      if close {
        return;
      }
    }
  });
  let deno_writer = tokio::spawn(async move {
    while let Some(owned) = deno_recv.recv().await {
      let close = owned.opcode == OpCode::Close;
      if let Err(err) = deno_tx.write_frame(owned.into_frame()).await {
        log::debug!("[devtools-mux] unified deno write: {err:?}");
        return;
      }
      if close {
        return;
      }
    }
  });

  // Reader: client → CEF/Deno (with CDP-aware routing).
  let mut client_reader = {
    let client_send = client_send.clone();
    let cef_send = cef_send.clone();
    let deno_send = deno_send.clone();
    let session_id = session_id.clone();
    let deno_announced = deno_announced.clone();
    tokio::spawn(async move {
      let mut noop = |_: Frame<'_>| async { Ok::<(), WebSocketError>(()) };
      loop {
        let frame = match client_rx.read_frame(&mut noop).await {
          Ok(f) => f,
          Err(err) => {
            log::debug!("[devtools-mux] unified client read: {err:?}");
            return;
          }
        };
        let opcode = frame.opcode;
        let payload = frame.payload.to_vec();
        match opcode {
          OpCode::Text => {
            route_client_text(
              &payload,
              &session_id,
              &client_send,
              &cef_send,
              &deno_send,
              &deno_announced,
            );
          }
          OpCode::Close => {
            let _ = cef_send.send(OwnedFrame {
              opcode,
              payload: payload.clone(),
            });
            let _ = deno_send.send(OwnedFrame { opcode, payload });
            return;
          }
          _ => {
            // Binary, Ping, Pong, Continuation: forward to CEF (the
            // primary session). Ping/pong on the client connection is
            // for keep-alive; CEF will reply.
            let _ = cef_send.send(OwnedFrame { opcode, payload });
          }
        }
      }
    })
  };

  // Reader: CEF → client. No sessionId injection, but we do rewrite
  // the execution-context name so the Console dropdown reads
  // "Renderer" instead of V8's default "top".
  let mut cef_reader = {
    let client_send = client_send.clone();
    tokio::spawn(async move {
      let mut noop = |_: Frame<'_>| async { Ok::<(), WebSocketError>(()) };
      loop {
        let frame = match cef_rx.read_frame(&mut noop).await {
          Ok(f) => f,
          Err(err) => {
            log::debug!("[devtools-mux] unified cef read: {err:?}");
            return;
          }
        };
        let opcode = frame.opcode;
        let payload = frame.payload.to_vec();
        let owned = if opcode == OpCode::Text {
          OwnedFrame::text(rewrite_text_from_upstream(
            &payload, None, "Renderer",
          ))
        } else {
          OwnedFrame { opcode, payload }
        };
        if client_send.send(owned).is_err() {
          return;
        }
      }
    })
  };

  // Reader: Deno → client. Inject sessionId so DevTools routes frames
  // to the synthetic child target, and relabel the execution context
  // to "Deno" instead of V8's "main realm".
  let mut deno_reader = {
    let client_send = client_send.clone();
    let session_id = session_id.clone();
    tokio::spawn(async move {
      let mut noop = |_: Frame<'_>| async { Ok::<(), WebSocketError>(()) };
      loop {
        let frame = match deno_rx.read_frame(&mut noop).await {
          Ok(f) => f,
          Err(err) => {
            log::debug!("[devtools-mux] unified deno read: {err:?}");
            return;
          }
        };
        let opcode = frame.opcode;
        let payload = frame.payload.to_vec();
        let owned = if opcode == OpCode::Text {
          OwnedFrame::text(rewrite_text_from_upstream(
            &payload,
            Some(&session_id),
            "Deno",
          ))
        } else {
          OwnedFrame { opcode, payload }
        };
        if client_send.send(owned).is_err() {
          return;
        }
      }
    })
  };

  // Drop the originals so writers exit once readers finish.
  drop(client_send);
  drop(cef_send);
  drop(deno_send);

  // Wait for any reader to exit, then tear down everything else.
  tokio::select! {
    _ = &mut client_reader => {},
    _ = &mut cef_reader => {},
    _ = &mut deno_reader => {},
  }

  client_reader.abort();
  cef_reader.abort();
  deno_reader.abort();
  client_writer.abort();
  cef_writer.abort();
  deno_writer.abort();

  Ok(())
}

/// CDP-aware routing for a text frame coming from the DevTools client.
///
/// - `sessionId == deno_session_id` → strip and send to Deno.
/// - `Target.setAutoAttach` / `Target.setDiscoverTargets(true)` →
///   forward to CEF AND lazily emit our synthetic
///   `Target.attachedToTarget` for Deno (once). We piggyback on these
///   calls because they are the frontend's signal that it's ready to
///   process target events; firing earlier causes the event to be
///   silently dropped.
/// - `Target.attachToTarget(deno-id)` → reply locally with the
///   synthetic sessionId; never reaches CEF (which doesn't know it).
/// - `Target.detachFromTarget(deno-session)` → reply locally and
///   synthesize the corresponding `Target.detachedFromTarget` event.
/// - everything else → forward to CEF.
fn route_client_text(
  payload: &[u8],
  session_id: &str,
  client_send: &mpsc::UnboundedSender<OwnedFrame>,
  cef_send: &mpsc::UnboundedSender<OwnedFrame>,
  deno_send: &mpsc::UnboundedSender<OwnedFrame>,
  deno_announced: &Arc<std::sync::atomic::AtomicBool>,
) {
  let mut value: Value = match serde_json::from_slice(payload) {
    Ok(v) => v,
    Err(_) => {
      let _ = cef_send.send(OwnedFrame::text(payload.to_vec()));
      return;
    }
  };

  let session = value.get("sessionId").and_then(|v| v.as_str());
  if session == Some(session_id) {
    if let Some(obj) = value.as_object_mut() {
      obj.remove("sessionId");
    }
    let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| payload.to_vec());
    let _ = deno_send.send(OwnedFrame::text(bytes));
    return;
  }

  let id = value.get("id").and_then(|v| v.as_i64());
  let method = value
    .get("method")
    .and_then(|v| v.as_str())
    .map(str::to_owned);

  // The frontend is now listening for target events — announce Deno.
  if matches!(
    method.as_deref(),
    Some("Target.setAutoAttach") | Some("Target.setDiscoverTargets")
  ) && !deno_announced.swap(true, std::sync::atomic::Ordering::SeqCst)
  {
    let event = attached_to_target_event(session_id);
    let _ =
      client_send.send(OwnedFrame::text(serde_json::to_vec(&event).unwrap()));
  }

  if method.as_deref() == Some("Target.attachToTarget") {
    let target_id = value
      .get("params")
      .and_then(|p| p.get("targetId"))
      .and_then(|v| v.as_str());
    if target_id == Some(DENO_CHILD_TARGET_ID) {
      if let Some(rid) = id {
        let reply = json!({
          "id": rid,
          "result": { "sessionId": session_id },
        });
        let _ = client_send
          .send(OwnedFrame::text(serde_json::to_vec(&reply).unwrap()));
      }
      return;
    }
  }

  if method.as_deref() == Some("Target.detachFromTarget") {
    let detach_session = value
      .get("params")
      .and_then(|p| p.get("sessionId"))
      .and_then(|v| v.as_str());
    if detach_session == Some(session_id) {
      if let Some(rid) = id {
        let reply = json!({ "id": rid, "result": {} });
        let _ = client_send
          .send(OwnedFrame::text(serde_json::to_vec(&reply).unwrap()));
      }
      let event = json!({
        "method": "Target.detachedFromTarget",
        "params": {
          "sessionId": session_id,
          "targetId": DENO_CHILD_TARGET_ID,
        },
      });
      let _ =
        client_send.send(OwnedFrame::text(serde_json::to_vec(&event).unwrap()));
      return;
    }
  }

  let _ = cef_send.send(OwnedFrame::text(payload.to_vec()));
}

/// Rewrite a JSON CDP frame on its way from an upstream to the
/// DevTools client:
///
/// - Optionally inject `sessionId = session_id` so DevTools attributes
///   the frame to the synthetic Deno child target.
/// - Rename `Runtime.executionContextCreated.params.context.name` to
///   `context_name` so the Console "execution context" dropdown shows
///   a meaningful label instead of V8's defaults (`"top"`, `"main
///   realm"`).
///
/// If the payload isn't valid JSON, return it unchanged.
fn rewrite_text_from_upstream(
  payload: &[u8],
  inject_session_id: Option<&str>,
  context_name: &str,
) -> Vec<u8> {
  let mut value: Value = match serde_json::from_slice(payload) {
    Ok(v) => v,
    Err(_) => return payload.to_vec(),
  };
  let Some(obj) = value.as_object_mut() else {
    return payload.to_vec();
  };
  if let Some(sid) = inject_session_id {
    obj.insert("sessionId".to_string(), Value::String(sid.to_string()));
  }
  if obj.get("method").and_then(|v| v.as_str())
    == Some("Runtime.executionContextCreated")
  {
    if let Some(ctx) = obj
      .get_mut("params")
      .and_then(|v| v.get_mut("context"))
      .and_then(|v| v.as_object_mut())
    {
      ctx.insert("name".to_string(), Value::String(context_name.to_string()));
    }
  }
  serde_json::to_vec(&value).unwrap_or_else(|_| payload.to_vec())
}

/// Build a `Target.attachedToTarget` event advertising the Deno
/// runtime isolate as a child of the unified session.
fn attached_to_target_event(session_id: &str) -> Value {
  // `inspector.html` only renders attached child targets in the Sources
  // panel "Threads" sidebar when their type matches a known
  // worker-style kind (`worker`, `shared_worker`, `service_worker`).
  // We pick `worker` so the Deno runtime isolate shows up alongside
  // the CEF renderer's main thread.
  json!({
    "method": "Target.attachedToTarget",
    "params": {
      "sessionId": session_id,
      "targetInfo": {
        "targetId": DENO_CHILD_TARGET_ID,
        "type": "worker",
        "title": "Deno Runtime",
        "url": "deno://runtime",
        "attached": true,
        "canAccessOpener": false,
      },
      "waitingForDebugger": false,
    },
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rewrite_ws_host_forces_host() {
    let host: SocketAddr = "127.0.0.1:9230".parse().unwrap();
    assert_eq!(
      rewrite_ws_host("ws://0.0.0.0:9230/devtools/browser/abc", host),
      "ws://127.0.0.1:9230/devtools/browser/abc"
    );
    assert_eq!(
      rewrite_ws_host("ws://localhost/ws/deadbeef", host),
      "ws://127.0.0.1:9230/ws/deadbeef"
    );
  }

  fn test_config() -> MuxConfig {
    MuxConfig {
      listen: "127.0.0.1:9229".parse().unwrap(),
      deno_internal: "127.0.0.1:9230".parse().unwrap(),
      cef_internal: "127.0.0.1:9231".parse().unwrap(),
      inspect_brk: false,
      wait_for_debugger: false,
    }
  }

  #[test]
  fn target_path_round_trip() {
    let state = MuxState::new(test_config(), "127.0.0.1:9229".parse().unwrap());
    assert_eq!(state.target_for_path("/unified"), Some(TargetKind::Unified));
    assert_eq!(state.target_for_path("/deno"), Some(TargetKind::Deno));
    assert_eq!(state.target_for_path("/cef"), Some(TargetKind::Cef));
    assert_eq!(state.target_for_path("/bogus"), None);
  }

  // ── sessionId dispatch ────────────────────────────────────────────

  #[test]
  fn route_client_text_strips_session_and_forwards_to_deno() {
    let (client_tx, _client_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let (cef_tx, _cef_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let (deno_tx, mut deno_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let announced = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let session_id = "test-session-123";
    let msg = json!({
      "id": 1,
      "method": "Debugger.enable",
      "sessionId": session_id,
    });
    let payload = serde_json::to_vec(&msg).unwrap();

    route_client_text(
      &payload, session_id, &client_tx, &cef_tx, &deno_tx, &announced,
    );

    // Should arrive at Deno with sessionId stripped.
    let frame = deno_rx.try_recv().expect("expected frame on deno channel");
    let value: Value = serde_json::from_slice(&frame.payload).unwrap();
    assert_eq!(value.get("id").unwrap(), 1);
    assert_eq!(value.get("method").unwrap(), "Debugger.enable");
    assert!(
      value.get("sessionId").is_none(),
      "sessionId should be stripped"
    );
  }

  #[test]
  fn route_client_text_forwards_non_session_to_cef() {
    let (client_tx, _client_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let (cef_tx, mut cef_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let (deno_tx, _deno_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let announced = Arc::new(std::sync::atomic::AtomicBool::new(true));

    let msg = json!({"id": 5, "method": "DOM.getDocument"});
    let payload = serde_json::to_vec(&msg).unwrap();

    route_client_text(
      &payload,
      "some-session",
      &client_tx,
      &cef_tx,
      &deno_tx,
      &announced,
    );

    let frame = cef_rx.try_recv().expect("expected frame on cef channel");
    let value: Value = serde_json::from_slice(&frame.payload).unwrap();
    assert_eq!(value.get("id").unwrap(), 5);
  }

  #[test]
  fn route_client_text_attach_to_deno_target_replies_locally() {
    let (client_tx, mut client_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let (cef_tx, mut cef_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let (deno_tx, _deno_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let announced = Arc::new(std::sync::atomic::AtomicBool::new(true));

    let session_id = "deno-sess";
    let msg = json!({
      "id": 10,
      "method": "Target.attachToTarget",
      "params": { "targetId": DENO_CHILD_TARGET_ID },
    });
    let payload = serde_json::to_vec(&msg).unwrap();

    route_client_text(
      &payload, session_id, &client_tx, &cef_tx, &deno_tx, &announced,
    );

    // Should reply to client with the synthetic sessionId.
    let frame = client_rx.try_recv().expect("expected reply on client");
    let value: Value = serde_json::from_slice(&frame.payload).unwrap();
    assert_eq!(value["id"], 10);
    assert_eq!(value["result"]["sessionId"], session_id);

    // Should NOT have forwarded to CEF.
    assert!(cef_rx.try_recv().is_err());
  }

  #[test]
  fn route_client_text_lazily_announces_deno() {
    let (client_tx, mut client_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let (cef_tx, _cef_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let (deno_tx, _deno_rx) = mpsc::unbounded_channel::<OwnedFrame>();
    let announced = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let msg = json!({
      "id": 1,
      "method": "Target.setAutoAttach",
      "params": { "autoAttach": true, "waitForDebuggerOnStart": false },
    });
    let payload = serde_json::to_vec(&msg).unwrap();

    route_client_text(
      &payload, "sess", &client_tx, &cef_tx, &deno_tx, &announced,
    );

    // Should have emitted Target.attachedToTarget event.
    let frame = client_rx.try_recv().expect("expected announce event");
    let value: Value = serde_json::from_slice(&frame.payload).unwrap();
    assert_eq!(value["method"], "Target.attachedToTarget");
    assert_eq!(value["params"]["targetInfo"]["type"], "worker");
    assert!(announced.load(std::sync::atomic::Ordering::SeqCst));

    // Calling again should NOT emit a second event.
    route_client_text(
      &payload, "sess", &client_tx, &cef_tx, &deno_tx, &announced,
    );
    // Only the forwarded-to-cef frame, no second announce.
    assert!(client_rx.try_recv().is_err());
  }

  // ── context name rewriting ────────────────────────────────────────

  #[test]
  fn rewrite_injects_session_id() {
    let input = json!({"id": 1, "result": {}});
    let payload = serde_json::to_vec(&input).unwrap();
    let out = rewrite_text_from_upstream(&payload, Some("my-sess"), "Deno");
    let value: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["sessionId"], "my-sess");
  }

  #[test]
  fn rewrite_renames_execution_context() {
    let input = json!({
      "method": "Runtime.executionContextCreated",
      "params": {
        "context": {
          "id": 1,
          "origin": "",
          "name": "top",
        },
      },
    });
    let payload = serde_json::to_vec(&input).unwrap();
    let out = rewrite_text_from_upstream(&payload, None, "Renderer");
    let value: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["params"]["context"]["name"], "Renderer");
  }

  #[test]
  fn rewrite_no_session_leaves_field_absent() {
    let input = json!({"method": "Console.messageAdded"});
    let payload = serde_json::to_vec(&input).unwrap();
    let out = rewrite_text_from_upstream(&payload, None, "Renderer");
    let value: Value = serde_json::from_slice(&out).unwrap();
    assert!(value.get("sessionId").is_none());
  }

  // ── /json/list shape ──────────────────────────────────────────────

  #[tokio::test]
  async fn json_list_returns_three_targets() {
    let state = MuxState::new(test_config(), "127.0.0.1:9229".parse().unwrap());
    let resp = json_list(&state).await;
    let body = resp.into_body();
    let bytes = body.collect().await.unwrap().to_bytes();
    let list: Vec<Value> = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(list.len(), 3);

    // Unified target.
    assert_eq!(list[0]["type"], "page");
    assert_eq!(list[0]["title"], "Deno Desktop (unified)");
    assert!(
      list[0]["webSocketDebuggerUrl"]
        .as_str()
        .unwrap()
        .ends_with("/unified")
    );

    // Deno direct target.
    assert_eq!(list[1]["type"], "node");
    assert_eq!(list[1]["title"], "Deno Runtime");
    assert!(
      list[1]["webSocketDebuggerUrl"]
        .as_str()
        .unwrap()
        .ends_with("/deno")
    );

    // CEF direct target.
    assert_eq!(list[2]["type"], "page");
    assert_eq!(list[2]["title"], "CEF Renderer");
    assert!(
      list[2]["webSocketDebuggerUrl"]
        .as_str()
        .unwrap()
        .ends_with("/cef")
    );
  }
}
