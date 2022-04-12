// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use core::convert::Infallible as Never; // Alias for the future `!` type.
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::futures::channel::mpsc::UnboundedReceiver;
use deno_core::futures::channel::mpsc::UnboundedSender;
use deno_core::futures::channel::oneshot;
use deno_core::futures::future;
use deno_core::futures::future::Future;
use deno_core::futures::pin_mut;
use deno_core::futures::prelude::*;
use deno_core::futures::select;
use deno_core::futures::stream::StreamExt;
use deno_core::futures::task::Poll;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::InspectorMsg;
use deno_core::InspectorSessionProxy;
use deno_core::JsRuntime;
use deno_websocket::tokio_tungstenite::tungstenite;
use deno_websocket::tokio_tungstenite::WebSocketStream;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
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
  pub fn new(host: SocketAddr, name: String) -> Self {
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
    should_break_on_first_statement: bool,
  ) {
    let inspector = js_runtime.inspector();
    let session_sender = inspector.get_session_sender();
    let deregister_rx = inspector.add_deregister_handler();
    let info = InspectorInfo::new(
      self.host,
      session_sender,
      deregister_rx,
      module_url,
      should_break_on_first_statement,
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
    tokio::task::spawn_local(fut);
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

  let resp = tungstenite::handshake::server::create_response(&req)
    .map(|resp| resp.map(|_| hyper::Body::empty()))
    .or_else(|e| match e {
      tungstenite::error::Error::HttpFormat(http_error) => Err(http_error),
      _ => http::Response::builder()
        .status(http::StatusCode::BAD_REQUEST)
        .body("Not a valid Websocket Request".into()),
    })?;

  let (parts, _) = req.into_parts();
  let req = http::Request::from_parts(parts, body);

  // spawn a task that will wait for websocket connection and then pump messages between
  // the socket and inspector proxy
  tokio::task::spawn_local(async move {
    let upgrade_result = hyper::upgrade::on(req).await;
    let upgraded = if let Ok(u) = upgrade_result {
      u
    } else {
      eprintln!("Inspector server failed to upgrade to WS connection");
      return;
    };
    let websocket = WebSocketStream::from_raw_socket(
      upgraded,
      tungstenite::protocol::Role::Server,
      None,
    )
    .await;

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
) -> http::Result<http::Response<hyper::Body>> {
  let data = inspector_map
    .borrow()
    .values()
    .map(|info| info.get_json_metadata())
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
  name: String,
) {
  let inspector_map_ =
    Rc::new(RefCell::new(HashMap::<Uuid, InspectorInfo>::new()));

  let inspector_map = Rc::clone(&inspector_map_);
  let register_inspector_handler = register_inspector_rx
    .map(|info| {
      eprintln!(
        "Debugger listening on {}",
        info.get_websocket_debugger_url()
      );
      eprintln!("Visit chrome://inspect to connect to the debugger.");
      if info.should_break_on_first_statement {
        eprintln!("Deno is waiting for debugger to connect.");
      }
      if inspector_map.borrow_mut().insert(info.uuid, info).is_some() {
        panic!("Inspector UUID already in map");
      }
    })
    .collect::<()>();

  let inspector_map = Rc::clone(&inspector_map_);
  let deregister_inspector_handler = future::poll_fn(|cx| {
    inspector_map
      .borrow_mut()
      .retain(|_, info| info.deregister_rx.poll_unpin(cx) == Poll::Pending);
    Poll::<Never>::Pending
  })
  .fuse();

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
          match (req.method(), req.uri().path()) {
            (&http::Method::GET, path) if path.starts_with("/ws/") => {
              handle_ws_request(req, inspector_map.clone())
            }
            (&http::Method::GET, "/json/version") => {
              handle_json_version_request(json_version_response.clone())
            }
            (&http::Method::GET, "/json") => {
              handle_json_request(inspector_map.clone())
            }
            (&http::Method::GET, "/json/list") => {
              handle_json_request(inspector_map.clone())
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
  let server_handler = hyper::server::Builder::new(
    hyper::server::conn::AddrIncoming::bind(&host).unwrap_or_else(|e| {
      eprintln!("Cannot start inspector server: {}.", e);
      process::exit(1);
    }),
    hyper::server::conn::Http::new().with_executor(LocalExecutor),
  )
  .serve(make_svc)
  .with_graceful_shutdown(async {
    shutdown_server_rx.await.ok();
  })
  .unwrap_or_else(|err| {
    eprintln!("Cannot start inspector server: {}.", err);
    process::exit(1);
  })
  .fuse();

  pin_mut!(register_inspector_handler);
  pin_mut!(deregister_inspector_handler);
  pin_mut!(server_handler);

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
  websocket: WebSocketStream<hyper::upgrade::Upgraded>,
  inbound_tx: UnboundedSender<String>,
  outbound_rx: UnboundedReceiver<InspectorMsg>,
) {
  let (websocket_tx, websocket_rx) = websocket.split();

  let outbound_pump = outbound_rx
    .map(|msg| tungstenite::Message::text(msg.content))
    .map(Ok)
    .forward(websocket_tx)
    .map_err(|_| ());

  let inbound_pump = async move {
    let _result = websocket_rx
      .map_err(AnyError::from)
      .map_ok(|msg| {
        // Messages that cannot be converted to strings are ignored.
        if let Ok(msg_text) = msg.into_text() {
          let _ = inbound_tx.unbounded_send(msg_text);
        }
      })
      .try_collect::<()>()
      .await;

    // Users don't care if there was an error coming from debugger,
    // just about the fact that debugger did disconnect.
    eprintln!("Debugger session ended");

    Ok(())
  };
  let _ = future::try_join(outbound_pump, inbound_pump).await;
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
  pub should_break_on_first_statement: bool,
}

impl InspectorInfo {
  pub fn new(
    host: SocketAddr,
    new_session_tx: mpsc::UnboundedSender<InspectorSessionProxy>,
    deregister_rx: oneshot::Receiver<()>,
    url: String,
    should_break_on_first_statement: bool,
  ) -> Self {
    Self {
      host,
      uuid: Uuid::new_v4(),
      thread_name: thread::current().name().map(|n| n.to_owned()),
      new_session_tx,
      deregister_rx,
      url,
      should_break_on_first_statement,
    }
  }

  fn get_json_metadata(&self) -> Value {
    json!({
      "description": "deno",
      "devtoolsFrontendUrl": self.get_frontend_url(),
      "faviconUrl": "https://deno.land/favicon.ico",
      "id": self.uuid.to_string(),
      "title": self.get_title(),
      "type": "node",
      "url": self.url.to_string(),
      "webSocketDebuggerUrl": self.get_websocket_debugger_url(),
    })
  }

  pub fn get_websocket_debugger_url(&self) -> String {
    format!("ws://{}/ws/{}", &self.host, &self.uuid)
  }

  fn get_frontend_url(&self) -> String {
    format!(
        "devtools://devtools/bundled/js_app.html?ws={}/ws/{}&experiments=true&v8only=true",
        &self.host, &self.uuid
      )
  }

  fn get_title(&self) -> String {
    format!(
      "deno{} [pid: {}]",
      self
        .thread_name
        .as_ref()
        .map(|n| format!(" - {}", n))
        .unwrap_or_default(),
      process::id(),
    )
  }
}
