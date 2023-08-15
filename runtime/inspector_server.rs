// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// Alias for the future `!` type.
use core::convert::Infallible as Never;
use deno_core::futures::channel::mpsc;
use deno_core::futures::channel::mpsc::UnboundedReceiver;
use deno_core::futures::channel::mpsc::UnboundedSender;
use deno_core::futures::channel::oneshot;
use deno_core::futures::future;
use deno_core::futures::future::Future;
use deno_core::futures::prelude::*;
use deno_core::futures::select;
use deno_core::futures::stream::StreamExt;
use deno_core::futures::task::Poll;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::task::spawn;
use deno_core::url::Url;
use deno_core::InspectorMsg;
use deno_core::InspectorSessionProxy;
use deno_core::JsRuntime;
use fastwebsockets::Frame;
use fastwebsockets::OpCode;
use fastwebsockets::WebSocket;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::pin;
use std::process;
use std::rc::Rc;
use std::thread;
use uuid::Uuid;

/// Websocket server that is used to proxy connections from
/// devtools to the inspector.
pub struct InspectorServer {
  pub host: SocketAddr,
  register_inspector_tx: UnboundedSender<InspectorInfo>,
  shutdown_server_tx: Option<oneshot::Sender<()>>,
  thread_handle: Option<thread::JoinHandle<()>>,
}

impl InspectorServer {
  pub fn new(host: SocketAddr, name: &'static str) -> Self {
    let (register_inspector_tx, register_inspector_rx) =
      mpsc::unbounded::<InspectorInfo>();

    let (shutdown_server_tx, shutdown_server_rx) = oneshot::channel();

    let thread_handle = thread::spawn(move || {
      let rt = crate::tokio_util::create_basic_runtime();
      let local = tokio::task::LocalSet::new();
      local.block_on(
        &rt,
        server(host, register_inspector_rx, shutdown_server_rx, name),
      )
    });

    Self {
      host,
      register_inspector_tx,
      shutdown_server_tx: Some(shutdown_server_tx),
      thread_handle: Some(thread_handle),
    }
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

// Needed so hyper can use non Send futures
#[derive(Clone)]
struct LocalExecutor;

impl<Fut> hyper::rt::Executor<Fut> for LocalExecutor
where
  Fut: Future + 'static,
  Fut::Output: 'static,
{
  fn execute(&self, fut: Fut) {
    deno_core::task::spawn(fut);
  }
}

fn handle_ws_request(
  req: http::Request<hyper::Body>,
  inspector_map_rc: Rc<RefCell<HashMap<Uuid, InspectorInfo>>>,
) -> http::Result<http::Response<hyper::Body>> {
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
      .body("Malformed inspector UUID".into());
  }

  // run in a block to not hold borrow to `inspector_map` for too long
  let new_session_tx = {
    let inspector_map = inspector_map_rc.borrow();
    let maybe_inspector_info = inspector_map.get(&maybe_uuid.unwrap());

    if maybe_inspector_info.is_none() {
      return http::Response::builder()
        .status(http::StatusCode::NOT_FOUND)
        .body("Invalid inspector UUID".into());
    }

    let info = maybe_inspector_info.unwrap();
    info.new_session_tx.clone()
  };
  let (parts, _) = req.into_parts();
  let mut req = http::Request::from_parts(parts, body);

  let (resp, fut) = match fastwebsockets::upgrade::upgrade(&mut req) {
    Ok(e) => e,
    _ => {
      return http::Response::builder()
        .status(http::StatusCode::BAD_REQUEST)
        .body("Not a valid Websocket Request".into());
    }
  };

  // spawn a task that will wait for websocket connection and then pump messages between
  // the socket and inspector proxy
  spawn(async move {
    let websocket = if let Ok(w) = fut.await {
      w
    } else {
      eprintln!("Inspector server failed to upgrade to WS connection");
      return;
    };

    // The 'outbound' channel carries messages sent to the websocket.
    let (outbound_tx, outbound_rx) = mpsc::unbounded();
    // The 'inbound' channel carries messages received from the websocket.
    let (inbound_tx, inbound_rx) = mpsc::unbounded();

    let inspector_session_proxy = InspectorSessionProxy {
      tx: outbound_tx,
      rx: inbound_rx,
    };

    eprintln!("Debugger session started.");
    let _ = new_session_tx.unbounded_send(inspector_session_proxy);
    pump_websocket_messages(websocket, inbound_tx, outbound_rx).await;
  });

  Ok(resp)
}

fn handle_json_request(
  inspector_map: Rc<RefCell<HashMap<Uuid, InspectorInfo>>>,
  host: Option<String>,
) -> http::Result<http::Response<hyper::Body>> {
  let data = inspector_map
    .borrow()
    .values()
    .map(move |info| info.get_json_metadata(&host))
    .collect::<Vec<_>>();
  http::Response::builder()
    .status(http::StatusCode::OK)
    .header(http::header::CONTENT_TYPE, "application/json")
    .body(serde_json::to_string(&data).unwrap().into())
}

fn handle_json_version_request(
  version_response: Value,
) -> http::Result<http::Response<hyper::Body>> {
  http::Response::builder()
    .status(http::StatusCode::OK)
    .header(http::header::CONTENT_TYPE, "application/json")
    .body(serde_json::to_string(&version_response).unwrap().into())
}

async fn server(
  host: SocketAddr,
  register_inspector_rx: UnboundedReceiver<InspectorInfo>,
  shutdown_server_rx: oneshot::Receiver<()>,
  name: &str,
) {
  let inspector_map_ =
    Rc::new(RefCell::new(HashMap::<Uuid, InspectorInfo>::new()));

  let inspector_map = Rc::clone(&inspector_map_);
  let mut register_inspector_handler = pin!(register_inspector_rx
    .map(|info| {
      eprintln!(
        "Debugger listening on {}",
        info.get_websocket_debugger_url(&info.host.to_string())
      );
      eprintln!("Visit chrome://inspect to connect to the debugger.");
      if info.wait_for_session {
        eprintln!("Deno is waiting for debugger to connect.");
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
    "V8-Version": deno_core::v8_version(),
  });

  let make_svc = hyper::service::make_service_fn(|_| {
    let inspector_map = Rc::clone(&inspector_map_);
    let json_version_response = json_version_response.clone();

    future::ok::<_, Infallible>(hyper::service::service_fn(
      move |req: http::Request<hyper::Body>| {
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
              .body("Not Found".into()),
          }
        })
      },
    ))
  });

  // Create the server manually so it can use the Local Executor
  let mut server_handler = pin!(hyper::server::Builder::new(
    hyper::server::conn::AddrIncoming::bind(&host).unwrap_or_else(|e| {
      eprintln!("Cannot start inspector server: {e}.");
      process::exit(1);
    }),
    hyper::server::conn::Http::new().with_executor(LocalExecutor),
  )
  .serve(make_svc)
  .with_graceful_shutdown(async {
    shutdown_server_rx.await.ok();
  })
  .unwrap_or_else(|err| {
    eprintln!("Cannot start inspector server: {err}.");
    process::exit(1);
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
  mut websocket: WebSocket<hyper::upgrade::Upgraded>,
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
                    eprintln!("Debugger session ended");
                    break 'pump;
                }
                _ => {
                    // Ignore other messages.
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
