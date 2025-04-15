// Copyright 2018-2025 the Deno authors. MIT license.

// Alias for the future `!` type.
use core::convert::Infallible as Never;
use std::cell::RefCell;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::pin;
use std::process;
use std::rc::Rc;
use std::task::Poll;
use std::thread;

use deno_core::futures::channel::mpsc;
use deno_core::futures::channel::mpsc::UnboundedReceiver;
use deno_core::futures::channel::mpsc::UnboundedSender;
use deno_core::futures::channel::oneshot;
use deno_core::futures::future;
use deno_core::futures::prelude::*;
use deno_core::futures::select;
use deno_core::futures::stream::StreamExt;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::unsync::spawn;
use deno_core::url::Url;
use deno_core::InspectorMsg;
use deno_core::InspectorSessionKind;
use deno_core::InspectorSessionOptions;
use deno_core::InspectorSessionProxy;
use deno_core::JsRuntime;
use fastwebsockets::Frame;
use fastwebsockets::OpCode;
use fastwebsockets::WebSocket;
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Websocket server that is used to proxy connections from
/// devtools to the inspector.
pub struct InspectorServer {
  pub host: SocketAddr,
  register_inspector_tx: UnboundedSender<InspectorInfo>,
  shutdown_server_tx: Option<broadcast::Sender<()>>,
  thread_handle: Option<thread::JoinHandle<()>>,
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
}

impl InspectorServer {
  pub fn new(
    host: SocketAddr,
    name: &'static str,
  ) -> Result<Self, InspectorServerError> {
    let (register_inspector_tx, register_inspector_rx) =
      mpsc::unbounded::<InspectorInfo>();

    let (shutdown_server_tx, shutdown_server_rx) = broadcast::channel(1);

    let tcp_listener = std::net::TcpListener::bind(host)
      .map_err(|source| InspectorServerError::Connect { host, source })?;
    tcp_listener.set_nonblocking(true)?;

    let thread_handle = thread::spawn(move || {
      let rt = crate::tokio_util::create_basic_runtime();
      let local = tokio::task::LocalSet::new();
      local.block_on(
        &rt,
        server(
          tcp_listener,
          register_inspector_rx,
          shutdown_server_rx,
          name,
        ),
      )
    });

    Ok(Self {
      host,
      register_inspector_tx,
      shutdown_server_tx: Some(shutdown_server_tx),
      thread_handle: Some(thread_handle),
    })
  }

  pub fn register_inspector(
    &self,
    module_url: String,
    js_runtime: &mut JsRuntime,
    wait_for_session: bool,
  ) {
    let inspector_rc = js_runtime.inspector();
    let mut inspector = inspector_rc.borrow_mut();
    let session_sender = inspector.get_session_sender();
    let deregister_rx = inspector.add_deregister_handler();
    let info = InspectorInfo::new(
      self.host,
      session_sender,
      deregister_rx,
      module_url,
      wait_for_session,
    );
    self.register_inspector_tx.unbounded_send(info).unwrap();
  }
}

impl Drop for InspectorServer {
  fn drop(&mut self) {
    if let Some(shutdown_server_tx) = self.shutdown_server_tx.take() {
      shutdown_server_tx
        .send(())
        .expect("unable to send shutdown signal");
    }

    if let Some(thread_handle) = self.thread_handle.take() {
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

  let maybe_uuid = req
    .uri()
    .path()
    .strip_prefix("/ws/")
    .and_then(|s| Uuid::parse_str(s).ok());

  if maybe_uuid.is_none() {
    return http::Response::builder()
      .status(http::StatusCode::BAD_REQUEST)
      .body(Box::new(Bytes::from("Malformed inspector UUID").into()));
  }

  // run in a block to not hold borrow to `inspector_map` for too long
  let new_session_tx = {
    let inspector_map = inspector_map_rc.borrow();
    let maybe_inspector_info = inspector_map.get(&maybe_uuid.unwrap());

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

  let (resp, fut) = match fastwebsockets::upgrade::upgrade(&mut req) {
    Ok((resp, fut)) => {
      let (parts, _body) = resp.into_parts();
      let resp = http::Response::from_parts(
        parts,
        Box::new(http_body_util::Full::new(Bytes::new())),
      );
      (resp, fut)
    }
    _ => {
      return http::Response::builder()
        .status(http::StatusCode::BAD_REQUEST)
        .body(Box::new(
          Bytes::from("Not a valid Websocket Request").into(),
        ));
    }
  };

  // spawn a task that will wait for websocket connection and then pump messages between
  // the socket and inspector proxy
  spawn(async move {
    let websocket = match fut.await {
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
      tx: outbound_tx,
      rx: inbound_rx,
      options: InspectorSessionOptions {
        kind: InspectorSessionKind::NonBlocking {
          wait_for_disconnect: true,
        },
      },
    };

    log::info!("Debugger session started.");
    let _ = new_session_tx.unbounded_send(inspector_session_proxy);
    pump_websocket_messages(websocket, inbound_tx, outbound_rx).await;
  });

  Ok(resp)
}

fn handle_json_request(
  inspector_map: Rc<RefCell<HashMap<Uuid, InspectorInfo>>>,
  host: Option<String>,
) -> http::Result<http::Response<Box<http_body_util::Full<Bytes>>>> {
  let data = inspector_map
    .borrow()
    .values()
    .map(move |info| info.get_json_metadata(&host))
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

async fn server(
  listener: std::net::TcpListener,
  register_inspector_rx: UnboundedReceiver<InspectorInfo>,
  shutdown_server_rx: broadcast::Receiver<()>,
  name: &str,
) {
  let inspector_map_ =
    Rc::new(RefCell::new(HashMap::<Uuid, InspectorInfo>::new()));

  let inspector_map = Rc::clone(&inspector_map_);
  let mut register_inspector_handler = pin!(register_inspector_rx
    .map(|info| {
      log::info!(
        "Debugger listening on {}",
        info.get_websocket_debugger_url(&info.host.to_string())
      );
      log::info!("Visit chrome://inspect to connect to the debugger.");
      if info.wait_for_session {
        log::info!("Deno is waiting for debugger to connect.");
      }
      if inspector_map.borrow_mut().insert(info.uuid, info).is_some() {
        panic!("Inspector UUID already in map");
      }
    })
    .collect::<()>());

  let inspector_map = Rc::clone(&inspector_map_);
  let mut deregister_inspector_handler = pin!(future::poll_fn(|cx| {
    inspector_map
      .borrow_mut()
      .retain(|_, info| info.deregister_rx.poll_unpin(cx) == Poll::Pending);
    Poll::<Never>::Pending
  })
  .fuse());

  let json_version_response = json!({
    "Browser": name,
    "Protocol-Version": "1.3",
    "V8-Version": deno_core::v8::VERSION_STRING,
  });

  // Create the server manually so it can use the Local Executor
  let listener = match TcpListener::from_std(listener) {
    Ok(l) => l,
    Err(err) => {
      log::error!("Cannot start inspector server: {:?}", err);
      return;
    }
  };

  let mut server_handler = pin!(deno_core::unsync::spawn(async move {
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

      let service = hyper::service::service_fn(
        move |req: http::Request<hyper::body::Incoming>| {
          future::ready({
            // If the host header can make a valid URL, use it
            let host = req
              .headers()
              .get("host")
              .and_then(|host| host.to_str().ok())
              .and_then(|host| Url::parse(&format!("http://{host}")).ok())
              .and_then(|url| match (url.host(), url.port()) {
                (Some(host), Some(port)) => Some(format!("{host}:{port}")),
                (Some(host), None) => Some(format!("{host}")),
                _ => None,
              });
            match (req.method(), req.uri().path()) {
              (&http::Method::GET, path) if path.starts_with("/ws/") => {
                handle_ws_request(req, Rc::clone(&inspector_map))
              }
              (&http::Method::GET, "/json/version") => {
                handle_json_version_request(json_version_response.clone())
              }
              (&http::Method::GET, "/json") => {
                handle_json_request(Rc::clone(&inspector_map), host)
              }
              (&http::Method::GET, "/json/list") => {
                handle_json_request(Rc::clone(&inspector_map), host)
              }
              _ => http::Response::builder()
                .status(http::StatusCode::NOT_FOUND)
                .body(Box::new(http_body_util::Full::new(Bytes::from(
                  "Not Found",
                )))),
            }
          })
        },
      );

      deno_core::unsync::spawn(async move {
        let server = hyper::server::conn::http1::Builder::new();

        let mut conn =
          pin!(server.serve_connection(io, service).with_upgrades());
        let mut shutdown_rx = pin!(shutdown_server_rx.recv());

        tokio::select! {
          result = conn.as_mut() => {
            if let Err(err) = result {
              log::error!("Failed to serve connection: {:?}", err);
            }
          },
          _ = &mut shutdown_rx => {
            conn.as_mut().graceful_shutdown();
            let _ = conn.await;
          }
        }
      });
    }
  })
  .fuse());

  select! {
    _ = register_inspector_handler => {},
    _ = deregister_inspector_handler => unreachable!(),
    _ = server_handler => {},
  }
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
        Some(msg) = outbound_rx.next() => {
            let msg = Frame::text(msg.content.into_bytes().into());
            let _ = websocket.write_frame(msg).await;
        }
        Ok(msg) = websocket.read_frame() => {
            match msg.opcode {
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
            }
        }
        else => {
          break 'pump;
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

  fn get_json_metadata(&self, host: &Option<String>) -> Value {
    let host_listen = format!("{}", self.host);
    let host = host.as_ref().unwrap_or(&host_listen);
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
    format!("ws://{}/ws/{}", host, &self.uuid)
  }

  fn get_frontend_url(&self, host: &str) -> String {
    format!(
        "devtools://devtools/bundled/js_app.html?ws={}/ws/{}&experiments=true&v8only=true",
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
