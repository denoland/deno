// Copyright 2018-2026 the Deno authors. MIT license.

// Alias for the future `!` type.
use core::convert::Infallible as Never;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::pin::pin;
use std::process;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;
use std::task::Poll;
use std::thread;

use deno_core::InspectorMsg;
use deno_core::InspectorSessionChannels;
use deno_core::InspectorSessionKind;
use deno_core::InspectorSessionProxy;
use deno_core::JsRuntimeInspector;
use deno_core::futures::channel::mpsc;
use deno_core::futures::channel::mpsc::UnboundedReceiver;
use deno_core::futures::channel::mpsc::UnboundedSender;
use deno_core::futures::channel::oneshot;
use deno_core::futures::future;
use deno_core::futures::prelude::*;
use deno_core::futures::stream::StreamExt;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_core::unsync::spawn;
use fastwebsockets::Frame;
use fastwebsockets::OpCode;
use fastwebsockets::WebSocket;
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use uuid::Uuid;

/// URL of the inspector server for a runtime instance.
/// This is used to check if the inspector is enabled and to get
/// the WebSocket URL for connecting to the debugger.
pub struct InspectorServerUrl(pub String);

/// Options for controlling where the inspector WebSocket URL is published.
/// Mirrors Node.js --inspect-publish-uid behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectPublishUid {
  /// Publish to stderr (console).
  pub console: bool,
  /// Publish via HTTP endpoint (/json, /json/list, /json/version).
  pub http: bool,
}

impl Default for InspectPublishUid {
  fn default() -> Self {
    Self {
      console: true,
      http: true,
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedHost {
  authority: String,
  hostname: String,
  port: Option<u16>,
}

impl ValidatedHost {
  pub fn authority(&self) -> &str {
    &self.authority
  }

  pub fn hostname(&self) -> &str {
    &self.hostname
  }

  pub fn port(&self) -> Option<u16> {
    self.port
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidHostHeader;

/// Validate an inspector request's `Host` header.
///
/// Inspector clients use `localhost` or an IP literal as their authority.
/// Keep the supplied port so discovery continues to work through port
/// forwarding, while rejecting named or ambiguous authorities.
pub fn validated_host_header<T>(
  req: &http::Request<T>,
) -> Result<Option<ValidatedHost>, InvalidHostHeader> {
  let mut values = req.headers().get_all(http::header::HOST).iter();
  let Some(value) = values.next() else {
    return Ok(None);
  };
  if values.next().is_some() {
    return Err(InvalidHostHeader);
  }
  let authority = value.to_str().map_err(|_| InvalidHostHeader)?;
  parse_host_authority(authority).map(Some)
}

fn parse_host_authority(
  authority: &str,
) -> Result<ValidatedHost, InvalidHostHeader> {
  fn parse_port(port: &str) -> Result<u16, InvalidHostHeader> {
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
      return Err(InvalidHostHeader);
    }
    port.parse().map_err(|_| InvalidHostHeader)
  }

  if authority.is_empty() {
    return Err(InvalidHostHeader);
  }

  if let Some(bracketed) = authority.strip_prefix('[') {
    let Some(end) = bracketed.find(']') else {
      return Err(InvalidHostHeader);
    };
    let hostname = &bracketed[..end];
    let suffix = &bracketed[end + 1..];
    let port = if suffix.is_empty() {
      None
    } else {
      Some(parse_port(
        suffix.strip_prefix(':').ok_or(InvalidHostHeader)?,
      )?)
    };
    let ip = hostname
      .parse::<Ipv6Addr>()
      .map_err(|_| InvalidHostHeader)?;
    if ip.is_unspecified() {
      return Err(InvalidHostHeader);
    }
    return Ok(ValidatedHost {
      authority: authority.to_string(),
      hostname: ip.to_string(),
      port,
    });
  }

  if authority.contains(['[', ']']) {
    return Err(InvalidHostHeader);
  }

  let (hostname, port) = match authority.rsplit_once(':') {
    Some((hostname, port)) => {
      if hostname.contains(':') {
        return Err(InvalidHostHeader);
      }
      (hostname, Some(parse_port(port)?))
    }
    None => (authority, None),
  };

  let hostname = if hostname.eq_ignore_ascii_case("localhost") {
    "localhost".to_string()
  } else {
    let ip = hostname
      .parse::<Ipv4Addr>()
      .map_err(|_| InvalidHostHeader)?;
    if ip.octets()[0] == 0 {
      return Err(InvalidHostHeader);
    }
    ip.to_string()
  };

  Ok(ValidatedHost {
    authority: authority.to_string(),
    hostname,
    port,
  })
}

/// Websocket server that is used to proxy connections from
/// devtools to the inspector.
pub struct InspectorServer {
  pub host: SocketAddr,
  register_inspector_tx: UnboundedSender<InspectorInfo>,
  shutdown_server_tx: Option<broadcast::Sender<()>>,
  /// Channel to signal an abrupt reset - closes all connections and deregisters all inspectors
  reset_tx: broadcast::Sender<()>,
  // Wrapped in Mutex to make InspectorServer Sync (JoinHandle is Send but not Sync)
  thread_handle: Mutex<Option<thread::JoinHandle<()>>>,
}

static GLOBAL_INSPECTOR_SERVER: OnceLock<Mutex<Option<Arc<InspectorServer>>>> =
  OnceLock::new();

fn global_server() -> &'static Mutex<Option<Arc<InspectorServer>>> {
  GLOBAL_INSPECTOR_SERVER.get_or_init(|| Mutex::new(None))
}

/// Returns the global inspector server if it has been created.
pub fn get_inspector_server() -> Option<Arc<InspectorServer>> {
  global_server().lock().clone()
}

/// Stops the global inspector server if it exists.
/// This abruptly closes all pending connections and deregisters all inspectors.
/// The server continues to run and can accept new connections/registrations.
pub fn stop_inspector_server() {
  if let Some(server) = global_server().lock().take() {
    server.stop();
  }
}

/// Creates a global inspector server at the given address with the given name.
/// Returns a reference to the server if created successfully, or the existing
/// server if one was already created (ignoring the provided parameters).
pub fn create_inspector_server(
  host: SocketAddr,
  name: &'static str,
  publish_uid: InspectPublishUid,
) -> Result<Arc<InspectorServer>, InspectorServerError> {
  let mut guard = global_server().lock();
  // Return existing server if already created
  if let Some(server) = guard.as_ref() {
    return Ok(server.clone());
  }

  let server = Arc::new(InspectorServer::new(host, name, publish_uid)?);
  *guard = Some(server.clone());
  Ok(server)
}

/// The default address the inspector server listens on, matching Node.js.
pub fn default_inspector_host() -> SocketAddr {
  SocketAddr::from(([127, 0, 0, 1], 9229))
}

/// Activates the global inspector server on the [`default_inspector_host`] and
/// registers `inspector`, mirroring Node.js' behavior when a process is sent
/// SIGUSR1.
///
/// This is a no-op returning `None` when a server is already running (an
/// `--inspect*` flag, `node:inspector`'s `open()`, or a previous activation).
/// On success the inspector's WebSocket URL is returned; if the server fails to
/// bind, the error is logged and `None` is returned.
pub fn activate_default_inspector_server(
  name: &'static str,
  module_url: String,
  inspector: Rc<JsRuntimeInspector>,
  wait_for_session: bool,
) -> Option<InspectorServerUrl> {
  if get_inspector_server().is_some() {
    return None;
  }
  match create_inspector_server(
    default_inspector_host(),
    name,
    InspectPublishUid::default(),
  ) {
    Ok(server) => {
      Some(server.register_inspector(module_url, inspector, wait_for_session))
    }
    Err(err) => {
      log::error!("Failed to start inspector server: {}", err);
      None
    }
  }
}

static RESTART_NOTIFIER: OnceLock<broadcast::Sender<()>> = OnceLock::new();

fn get_restart_notifier() -> broadcast::Sender<()> {
  RESTART_NOTIFIER
    .get_or_init(|| broadcast::channel(16).0)
    .clone()
}

/// Notifies all connected /ws/events clients that a restart is about to occur.
pub fn notify_restart() {
  let sender = get_restart_notifier();
  let _ = sender.send(()); // Ignore error if no receivers
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum InspectorServerError {
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error("Failed to start inspector server at \"{host}\"")]
  Connect {
    host: SocketAddr,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error("Failed to get inspector server's assigned address")]
  LocalAddr {
    host: SocketAddr,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
}

fn create_basic_runtime() -> tokio::runtime::Runtime {
  tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    .max_blocking_threads(4)
    .build()
    .unwrap()
}

impl InspectorServer {
  pub fn new(
    host: SocketAddr,
    name: &'static str,
    publish_uid: InspectPublishUid,
  ) -> Result<Self, InspectorServerError> {
    let (register_inspector_tx, register_inspector_rx) =
      mpsc::unbounded::<InspectorInfo>();

    let (shutdown_server_tx, shutdown_server_rx) = broadcast::channel(1);
    let (reset_tx, reset_rx) = broadcast::channel(1);

    let tcp_listener = std::net::TcpListener::bind(host)
      .map_err(|source| InspectorServerError::Connect { host, source })?;
    tcp_listener.set_nonblocking(true)?;
    // TODO(bartlomieju): update process wide inspector server host
    let host = tcp_listener
      .local_addr()
      .map_err(|source| InspectorServerError::LocalAddr { host, source })?;

    let thread_handle = thread::spawn(move || {
      let rt = create_basic_runtime();
      let local = tokio::task::LocalSet::new();
      local.block_on(
        &rt,
        server(
          tcp_listener,
          register_inspector_rx,
          shutdown_server_rx,
          reset_rx,
          name,
          publish_uid,
        ),
      )
    });

    Ok(Self {
      host,
      register_inspector_tx,
      shutdown_server_tx: Some(shutdown_server_tx),
      reset_tx,
      thread_handle: Mutex::new(Some(thread_handle)),
    })
  }

  pub fn register_inspector(
    &self,
    module_url: String,
    inspector: Rc<JsRuntimeInspector>,
    wait_for_session: bool,
  ) -> InspectorServerUrl {
    let session_sender = inspector.get_session_sender();
    let deregister_rx = inspector.add_deregister_handler();
    let info = InspectorInfo::new(
      self.host,
      session_sender,
      deregister_rx,
      module_url,
      wait_for_session,
    );
    let url = InspectorServerUrl(
      info.get_websocket_debugger_url(&self.host.to_string()),
    );
    self.register_inspector_tx.unbounded_send(info).unwrap();
    url
  }

  /// Stop the inspector server by resetting IO.
  /// This abruptly closes all pending connections and deregisters all inspectors.
  pub fn stop(&self) {
    let _ = self.reset_tx.send(());
  }
}

impl Drop for InspectorServer {
  fn drop(&mut self) {
    if let Some(shutdown_server_tx) = self.shutdown_server_tx.take() {
      // The server thread may have already terminated (e.g. after a prior
      // `reset_tx.send(())` from `stop()` caused `server()` to return),
      // in which case the broadcast receiver is gone. Treat that as a
      // no-op rather than panicking during process teardown.
      let _ = shutdown_server_tx.send(());
    }

    if let Some(thread_handle) = self.thread_handle.lock().take() {
      thread_handle.join().expect("unable to join thread");
    }
  }
}

fn handle_ws_request(
  req: http::Request<hyper::body::Incoming>,
  inspector_map_rc: Rc<RefCell<HashMap<Uuid, InspectorInfo>>>,
) -> http::Result<http::Response<Box<http_body_util::Full<Bytes>>>> {
  let (parts, body) = req.into_parts();
  let req = http::Request::from_parts(parts, ());

  // Accept both /UUID (Node.js-compatible format) and /ws/UUID (legacy
  // format) so existing clients keep working while new ones can use the
  // shorter Node.js-style URL.
  let path = req.uri().path();
  let maybe_uuid = path
    .strip_prefix("/ws/")
    .or_else(|| path.strip_prefix('/'))
    .and_then(|s| Uuid::parse_str(s).ok());

  let Some(uuid) = maybe_uuid else {
    return http::Response::builder()
      .status(http::StatusCode::BAD_REQUEST)
      .body(Box::new(Bytes::from("Malformed inspector UUID").into()));
  };

  // run in a block to not hold borrow to `inspector_map` for too long
  let new_session_tx = {
    let inspector_map = inspector_map_rc.borrow();
    let maybe_inspector_info = inspector_map.get(&uuid);

    if maybe_inspector_info.is_none() {
      return http::Response::builder()
        .status(http::StatusCode::NOT_FOUND)
        .body(Box::new(Bytes::from("Invalid inspector UUID").into()));
    }

    let info = maybe_inspector_info.unwrap();
    info.new_session_tx.clone()
  };
  let (parts, _) = req.into_parts();
  let mut req = http::Request::from_parts(parts, body);

  let Ok((resp, upgrade_fut)) = fastwebsockets::upgrade::upgrade(&mut req)
  else {
    return http::Response::builder()
      .status(http::StatusCode::BAD_REQUEST)
      .body(Box::new(
        Bytes::from("Not a valid Websocket Request").into(),
      ));
  };

  // spawn a task that will wait for websocket connection and then pump messages between
  // the socket and inspector proxy
  spawn(async move {
    let mut websocket = match upgrade_fut.await {
      Ok(w) => w,
      Err(err) => {
        log::error!(
          "Inspector server failed to upgrade to WS connection: {:?}",
          err
        );
        return;
      }
    };

    // The 'outbound' channel carries messages sent to the websocket.
    let (outbound_tx, outbound_rx) = mpsc::unbounded();
    // The 'inbound' channel carries messages received from the websocket.
    let (inbound_tx, inbound_rx) = mpsc::unbounded();

    let inspector_session_proxy = InspectorSessionProxy {
      channels: InspectorSessionChannels::Regular {
        tx: outbound_tx,
        rx: inbound_rx,
      },
      kind: InspectorSessionKind::NonBlocking {
        wait_for_disconnect: true,
      },
    };

    if new_session_tx
      .unbounded_send(inspector_session_proxy)
      .is_err()
    {
      // The inspector was deregistered between the map lookup and now (e.g.
      // the runtime is being torn down for a watcher restart). Close the
      // socket instead of leaving the client attached to a session that will
      // never respond.
      close_going_away(&mut websocket).await;
      return;
    }
    log::info!("Debugger session started.");
    pump_websocket_messages(websocket, inbound_tx, outbound_rx).await;
  });

  let (parts, _body) = resp.into_parts();
  let resp = http::Response::from_parts(
    parts,
    Box::new(http_body_util::Full::new(Bytes::new())),
  );
  Ok(resp)
}

fn handle_json_request(
  inspector_map: Rc<RefCell<HashMap<Uuid, InspectorInfo>>>,
  host: Option<ValidatedHost>,
) -> http::Result<http::Response<Box<http_body_util::Full<Bytes>>>> {
  let data = inspector_map
    .borrow()
    .values()
    .map(move |info| {
      info.get_json_metadata(host.as_ref().map(ValidatedHost::authority))
    })
    .collect::<Vec<_>>();
  let body: http_body_util::Full<Bytes> =
    Bytes::from(serde_json::to_string(&data).unwrap()).into();
  http::Response::builder()
    .status(http::StatusCode::OK)
    .header(http::header::CONTENT_TYPE, "application/json")
    .body(Box::new(body))
}

fn handle_json_version_request(
  version_response: Value,
) -> http::Result<http::Response<Box<http_body_util::Full<Bytes>>>> {
  let body = Box::new(http_body_util::Full::from(
    serde_json::to_string(&version_response).unwrap(),
  ));

  http::Response::builder()
    .status(http::StatusCode::OK)
    .header(http::header::CONTENT_TYPE, "application/json")
    .body(body)
}

fn handle_ws_events_request(
  req: http::Request<hyper::body::Incoming>,
) -> http::Result<http::Response<Box<http_body_util::Full<Bytes>>>> {
  #[allow(clippy::disallowed_methods, reason = "TODO: pass a sys in here")]
  if std::env::var("UNSTABLE_INSPECTOR_WS_EVENTS").is_err() {
    return http::Response::builder()
      .status(http::StatusCode::NOT_FOUND)
      .body(Box::new(http_body_util::Full::new(Bytes::from(
        "Not Found",
      ))));
  }

  let (parts, body) = req.into_parts();
  let req = http::Request::from_parts(parts, ());

  let (parts, _) = req.into_parts();
  let mut req = http::Request::from_parts(parts, body);

  let Ok((resp, upgrade_fut)) = fastwebsockets::upgrade::upgrade(&mut req)
  else {
    return http::Response::builder()
      .status(http::StatusCode::BAD_REQUEST)
      .body(Box::new(http_body_util::Full::new(Bytes::from(
        "Not a valid Websocket Request",
      ))));
  };

  let restart_rx = get_restart_notifier().subscribe();

  // spawn a task that will wait for websocket connection and then pump event notifications
  spawn(async move {
    let websocket = match upgrade_fut.await {
      Ok(w) => w,
      Err(err) => {
        log::error!(
          "Inspector server failed to upgrade to WS connection for /ws/events: {:?}",
          err
        );
        return;
      }
    };

    log::debug!("Deno event session started.");
    pump_event_notifications(websocket, restart_rx).await;
  });

  let (parts, _body) = resp.into_parts();
  let resp = http::Response::from_parts(
    parts,
    Box::new(http_body_util::Full::new(Bytes::new())),
  );
  Ok(resp)
}

async fn pump_event_notifications(
  mut websocket: WebSocket<TokioIo<hyper::upgrade::Upgraded>>,
  mut restart_rx: broadcast::Receiver<()>,
) {
  loop {
    tokio::select! {
      result = restart_rx.recv() => {
        match result {
          Ok(()) => {
            #[allow(clippy::disallowed_methods, reason = "TODO: pass a sys in here")]
            let timestamp = std::time::SystemTime::now()
              .duration_since(std::time::UNIX_EPOCH)
              .map(|d| d.as_millis() as u64)
              .unwrap_or(0);
            let msg = json!({
              "type": "restart",
              "timestamp": timestamp,
            });
            let frame = Frame::text(msg.to_string().into_bytes().into());
            if websocket.write_frame(frame).await.is_err() {
              break;
            }
          }
          Err(broadcast::error::RecvError::Lagged(_)) => {
            // Missed some messages, continue
            continue;
          }
          Err(broadcast::error::RecvError::Closed) => {
            break;
          }
        }
      }
      result = websocket.read_frame() => {
        match result {
          Ok(frame) => {
            if frame.opcode == OpCode::Close {
              log::debug!("Deno event session ended");
              break;
            }
            // Ignore other messages
          }
          Err(_) => {
            break;
          }
        }
      }
    }
  }
}

async fn server(
  listener: std::net::TcpListener,
  register_inspector_rx: UnboundedReceiver<InspectorInfo>,
  shutdown_server_rx: broadcast::Receiver<()>,
  reset_rx: broadcast::Receiver<()>,
  name: &str,
  publish_uid: InspectPublishUid,
) {
  let inspector_map_ =
    Rc::new(RefCell::new(HashMap::<Uuid, InspectorInfo>::new()));

  let inspector_map = Rc::clone(&inspector_map_);
  let register_inspector_handler = listen_for_new_inspectors(
    register_inspector_rx,
    inspector_map.clone(),
    publish_uid,
  )
  .boxed_local();

  let inspector_map = Rc::clone(&inspector_map_);
  let mut reset_rx_deregister = reset_rx.resubscribe();
  let deregister_inspector_handler = future::poll_fn(|cx| {
    // Check for reset signal
    if let Ok(()) = reset_rx_deregister.try_recv() {
      // Clear all registered inspectors
      inspector_map.borrow_mut().clear();
    }
    inspector_map
      .borrow_mut()
      .retain(|_, info| info.deregister_rx.poll_unpin(cx) == Poll::Pending);
    Poll::<Never>::Pending
  })
  .boxed_local();

  let json_version_response = json!({
    "Browser": name,
    "Protocol-Version": "1.3",
    "V8-Version": deno_core::v8::VERSION_STRING,
  });

  let mut reset_rx_server = reset_rx.resubscribe();

  // Create the server manually so it can use the Local Executor
  let listener = match TcpListener::from_std(listener) {
    Ok(l) => l,
    Err(err) => {
      log::error!("Cannot start inspector server: {:?}", err);
      return;
    }
  };

  let server_handler = async move {
    loop {
      let mut rx = shutdown_server_rx.resubscribe();
      let mut shutdown_rx = pin!(rx.recv());
      let mut accept = pin!(listener.accept());

      let stream = tokio::select! {
        accept_result = &mut accept => {
          match accept_result {
            Ok((s, _)) => s,
            Err(err) => {
              log::error!("Failed to accept inspector connection: {:?}", err);
              continue;
            }
          }
        },

        _ = &mut shutdown_rx => {
          break;
        }
      };
      let io = TokioIo::new(stream);

      let inspector_map = Rc::clone(&inspector_map_);
      let json_version_response = json_version_response.clone();
      let mut shutdown_server_rx = shutdown_server_rx.resubscribe();
      let mut reset_rx_conn = reset_rx.resubscribe();

      let service = hyper::service::service_fn(
        move |req: http::Request<hyper::body::Incoming>| {
          future::ready(match validated_host_header(&req) {
            Ok(host) => match (req.method(), req.uri().path()) {
              (&http::Method::GET, "/ws/events") => {
                handle_ws_events_request(req)
              }
              (&http::Method::GET, path) if path.starts_with("/ws/") => {
                handle_ws_request(req, Rc::clone(&inspector_map))
              }
              (&http::Method::GET, "/json/version") if publish_uid.http => {
                handle_json_version_request(json_version_response.clone())
              }
              (&http::Method::GET, "/json") if publish_uid.http => {
                handle_json_request(Rc::clone(&inspector_map), host)
              }
              (&http::Method::GET, "/json/list") if publish_uid.http => {
                handle_json_request(Rc::clone(&inspector_map), host)
              }
              // Node.js-compatible "/UUID" path for the WebSocket session.
              (&http::Method::GET, path)
                if Uuid::parse_str(path.trim_start_matches('/')).is_ok() =>
              {
                handle_ws_request(req, Rc::clone(&inspector_map))
              }
              _ => http::Response::builder()
                .status(http::StatusCode::NOT_FOUND)
                .body(Box::new(http_body_util::Full::new(Bytes::from(
                  "Not Found",
                )))),
            },
            Err(_) => http::Response::builder()
              .status(http::StatusCode::BAD_REQUEST)
              .body(Box::new(http_body_util::Full::new(Bytes::from(
                "Invalid Host header",
              )))),
          })
        },
      );

      deno_core::unsync::spawn(async move {
        let server = hyper::server::conn::http1::Builder::new();

        let mut conn =
          pin!(server.serve_connection(io, service).with_upgrades());
        let mut shutdown_rx = pin!(shutdown_server_rx.recv());
        let mut reset_rx = pin!(reset_rx_conn.recv());

        tokio::select! {
          result = conn.as_mut() => {
            if let Err(err) = result {
              log::error!("Failed to serve connection: {:?}", err);
            }
          },
          _ = &mut shutdown_rx => {
            conn.as_mut().graceful_shutdown();
            let _ = conn.await;
          },
          _ = &mut reset_rx => {
            // Abruptly close the connection without graceful shutdown
          }
        }
      });
    }
  }
  .boxed_local();

  tokio::select! {
    _ = register_inspector_handler => {},
    _ = deregister_inspector_handler => unreachable!(),
    _ = server_handler => {},
    _ = reset_rx_server.recv() => {},
  }
}

async fn listen_for_new_inspectors(
  mut register_inspector_rx: UnboundedReceiver<InspectorInfo>,
  inspector_map: Rc<RefCell<HashMap<Uuid, InspectorInfo>>>,
  publish_uid: InspectPublishUid,
) {
  while let Some(info) = register_inspector_rx.next().await {
    if publish_uid.console {
      log::info!(
        "Debugger listening on {}",
        info.get_websocket_debugger_url(&info.host.to_string())
      );
      log::info!("Visit chrome://inspect to connect to the debugger.");
      if info.wait_for_session {
        log::info!("Deno is waiting for debugger to connect.");
      }
    }
    if inspector_map.borrow_mut().insert(info.uuid, info).is_some() {
      panic!("Inspector UUID already in map");
    }
  }
}

/// Closes the websocket with code 1001 ("going away"), telling the client
/// that the server-side session is gone, e.g. because the runtime was torn
/// down for a watcher restart or the process is exiting.
async fn close_going_away(
  websocket: &mut WebSocket<TokioIo<hyper::upgrade::Upgraded>>,
) {
  let _ = websocket
    .write_frame(Frame::close(1001, b"going away"))
    .await;
}

/// The pump future takes care of forwarding messages between the websocket
/// and channels. It resolves when either side disconnects, ignoring any
/// errors.
///
/// The future proxies messages sent and received on a warp WebSocket
/// to a UnboundedSender/UnboundedReceiver pair. We need these "unbounded" channel ends to sidestep
/// Tokio's task budget, which causes issues when JsRuntimeInspector::poll_sessions()
/// needs to block the thread because JavaScript execution is paused.
///
/// This works because UnboundedSender/UnboundedReceiver are implemented in the
/// 'futures' crate, therefore they can't participate in Tokio's cooperative
/// task yielding.
async fn pump_websocket_messages(
  mut websocket: WebSocket<TokioIo<hyper::upgrade::Upgraded>>,
  inbound_tx: UnboundedSender<String>,
  mut outbound_rx: UnboundedReceiver<InspectorMsg>,
) {
  'pump: loop {
    tokio::select! {
        maybe_msg = outbound_rx.next() => {
            match maybe_msg {
                Some(msg) => {
                    let msg = Frame::text(msg.content.into_bytes().into());
                    let _ = websocket.write_frame(msg).await;
                }
                None => {
                    // The isolate-side session was dropped without the client
                    // disconnecting first, e.g. the file watcher tore down the
                    // runtime while this server (a process-wide singleton)
                    // lives on. Close the socket so the client can detect the
                    // restart and reconnect to the new session; otherwise the
                    // connection stays open but silently ignores all messages.
                    close_going_away(&mut websocket).await;
                    log::info!("Debugger session ended (runtime was torn down)");
                    break 'pump;
                }
            }
        }
        result = websocket.read_frame() => {
            match result {
                Ok(msg) => match msg.opcode {
                    OpCode::Text => {
                        if let Ok(s) = String::from_utf8(msg.payload.to_vec()) {
                          let _ = inbound_tx.unbounded_send(s);
                        }
                    }
                    OpCode::Close => {
                        // Users don't care if there was an error coming from debugger,
                        // just about the fact that debugger did disconnect.
                        log::info!("Debugger session ended");
                        break 'pump;
                    }
                    _ => {
                        // Ignore other messages.
                    }
                },
                Err(_) => {
                    // WebSocket read error (remote disconnected without close frame).
                    log::info!("Debugger session ended (connection lost)");
                    break 'pump;
                }
            }
        }
    }
  }
}

/// Inspector information that is sent from the isolate thread to the server
/// thread when a new inspector is created.
pub struct InspectorInfo {
  pub host: SocketAddr,
  pub uuid: Uuid,
  pub thread_name: Option<String>,
  pub new_session_tx: UnboundedSender<InspectorSessionProxy>,
  pub deregister_rx: oneshot::Receiver<()>,
  pub url: String,
  pub wait_for_session: bool,
}

impl InspectorInfo {
  pub fn new(
    host: SocketAddr,
    new_session_tx: mpsc::UnboundedSender<InspectorSessionProxy>,
    deregister_rx: oneshot::Receiver<()>,
    url: String,
    wait_for_session: bool,
  ) -> Self {
    Self {
      host,
      uuid: Uuid::new_v4(),
      thread_name: thread::current().name().map(|n| n.to_owned()),
      new_session_tx,
      deregister_rx,
      url,
      wait_for_session,
    }
  }

  fn get_json_metadata(&self, host: Option<&str>) -> Value {
    let host_listen = format!("{}", self.host);
    let host = host.unwrap_or(&host_listen);
    json!({
      "description": "deno",
      "devtoolsFrontendUrl": self.get_frontend_url(host),
      "faviconUrl": "https://deno.land/favicon.ico",
      "id": self.uuid.to_string(),
      "title": self.get_title(),
      "type": "node",
      "url": self.url.to_string(),
      "webSocketDebuggerUrl": self.get_websocket_debugger_url(host),
    })
  }

  pub fn get_websocket_debugger_url(&self, host: &str) -> String {
    format!("ws://{}/{}", host, &self.uuid)
  }

  fn get_frontend_url(&self, host: &str) -> String {
    format!(
      "devtools://devtools/bundled/js_app.html?ws={}/{}&experiments=true&v8only=true",
      host, &self.uuid
    )
  }

  fn get_title(&self) -> String {
    format!(
      "deno{} [pid: {}]",
      self
        .thread_name
        .as_ref()
        .map(|n| format!(" - {n}"))
        .unwrap_or_default(),
      process::id(),
    )
  }
}

/// Channel for forwarding worker inspector session proxies to the main runtime.
/// Workers send their InspectorSessionProxy through this channel to establish
/// bidirectional debugging communication with the main inspector session.
pub struct MainInspectorSessionChannel(
  Arc<Mutex<Option<UnboundedSender<InspectorSessionProxy>>>>,
);

impl MainInspectorSessionChannel {
  pub fn new() -> Self {
    Self(Arc::new(Mutex::new(None)))
  }

  pub fn set(&self, tx: UnboundedSender<InspectorSessionProxy>) {
    *self.0.lock() = Some(tx);
  }

  pub fn get(&self) -> Option<UnboundedSender<InspectorSessionProxy>> {
    self.0.lock().clone()
  }
}

impl Default for MainInspectorSessionChannel {
  fn default() -> Self {
    Self::new()
  }
}

impl Clone for MainInspectorSessionChannel {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use http_body_util::BodyExt;
  use http_body_util::Empty;

  use super::*;

  fn request_with_host(host: Option<&str>) -> http::Request<()> {
    let mut request = http::Request::builder().uri("/json");
    if let Some(host) = host {
      request = request.header(http::header::HOST, host);
    }
    request.body(()).unwrap()
  }

  #[test]
  fn validates_unambiguous_inspector_authorities() {
    for authority in [
      "localhost",
      "LOCALHOST:43123",
      "127.0.0.1",
      "192.0.2.1:43123",
      "[::1]",
      "[2001:db8::1]:43123",
    ] {
      let request = request_with_host(Some(authority));
      let host = validated_host_header(&request).unwrap().unwrap();
      assert_eq!(host.authority(), authority);
    }
    assert!(
      validated_host_header(&request_with_host(None))
        .unwrap()
        .is_none()
    );
  }

  #[test]
  fn rejects_named_or_ambiguous_inspector_authorities() {
    for authority in [
      "",
      "example.test",
      "localhost.",
      "localhost.example",
      "user@localhost",
      "127.1",
      "0177.0.0.1",
      "0x7f.0.0.1",
      "2130706433",
      "0.0.0.0",
      "0.1.2.3:9229",
      "::1",
      "[::]",
      "[::1",
      "[::1]:",
      "[::1]:99999",
      "127.0.0.1:abc",
    ] {
      assert!(
        validated_host_header(&request_with_host(Some(authority))).is_err(),
        "{authority} should be rejected"
      );
    }

    let request = http::Request::builder()
      .uri("/json")
      .header(http::header::HOST, "localhost:9229")
      .header(http::header::HOST, "127.0.0.1:9229")
      .body(())
      .unwrap();
    assert!(validated_host_header(&request).is_err());
  }

  async fn send_http(
    addr: SocketAddr,
    request: http::Request<Empty<Bytes>>,
  ) -> (http::StatusCode, String) {
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let io = TokioIo::new(stream);
    let (mut sender, connection) =
      hyper::client::conn::http1::handshake(io).await.unwrap();
    tokio::spawn(async move {
      let _ = connection.with_upgrades().await;
    });
    let response = sender.send_request(request).await.unwrap();
    let status = response.status();
    let body = response.collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
  }

  fn get_request(
    path: &str,
    host: Option<&str>,
  ) -> http::Request<Empty<Bytes>> {
    let mut request =
      http::Request::builder().method(http::Method::GET).uri(path);
    if let Some(host) = host {
      request = request.header(http::header::HOST, host);
    } else {
      request = request.version(http::Version::HTTP_10);
    }
    request.body(Empty::new()).unwrap()
  }

  #[tokio::test]
  async fn inspector_routes_validate_host_and_preserve_forwarded_authority() {
    let server = InspectorServer::new(
      "127.0.0.1:0".parse().unwrap(),
      "deno",
      InspectPublishUid::default(),
    )
    .unwrap();

    let (new_session_tx, _new_session_rx) = mpsc::unbounded();
    let (_keep_registered, deregister_rx) = oneshot::channel();
    let info = InspectorInfo::new(
      server.host,
      new_session_tx,
      deregister_rx,
      "file:///main.ts".to_string(),
      false,
    );
    let uuid = info.uuid;
    server.register_inspector_tx.unbounded_send(info).unwrap();

    let mut forwarded_response = String::new();
    for _ in 0..50 {
      let (status, response) =
        send_http(server.host, get_request("/json", Some("localhost:43123")))
          .await;
      assert_eq!(status, http::StatusCode::OK);
      forwarded_response = response;
      if forwarded_response.contains(&uuid.to_string()) {
        break;
      }
      tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(forwarded_response.contains(&format!(
      "\"webSocketDebuggerUrl\":\"ws://localhost:43123/{uuid}\""
    )));

    let (status, ip_response) =
      send_http(server.host, get_request("/json", Some("127.0.0.1:54321")))
        .await;
    assert_eq!(status, http::StatusCode::OK);
    assert!(ip_response.contains(&format!(
      "\"webSocketDebuggerUrl\":\"ws://127.0.0.1:54321/{uuid}\""
    )));

    let (status, _) = send_http(
      server.host,
      get_request("/json", Some("example.test:43123")),
    )
    .await;
    assert_eq!(status, http::StatusCode::BAD_REQUEST);

    let ws_request = http::Request::builder()
      .method(http::Method::GET)
      .uri(format!("/ws/{uuid}"))
      .header(http::header::HOST, "example.test:43123")
      .header(http::header::UPGRADE, "websocket")
      .header(http::header::CONNECTION, "upgrade")
      .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
      .header("Sec-WebSocket-Version", "13")
      .body(Empty::new())
      .unwrap();
    let (status, _) = send_http(server.host, ws_request).await;
    assert_eq!(status, http::StatusCode::BAD_REQUEST);

    let (status, _) = send_http(server.host, get_request("/json", None)).await;
    assert_eq!(status, http::StatusCode::OK);
  }
}
