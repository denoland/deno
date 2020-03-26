#![allow(dead_code)]

use crate::futures::SinkExt;
use deno_core::v8;
use futures;
use futures::StreamExt;
use std::collections::HashMap;
use std::future::Future;
use std::mem::MaybeUninit;
use std::net::SocketAddrV4;
use std::pin::Pin;
use std::ptr;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use tokio;
use tokio::sync::mpsc;
use uuid::Uuid;
use warp;
use warp::filters::ws;
use warp::Filter;

const CONTEXT_GROUP_ID: i32 = 1;

// These messages can be sent from any thread to the server thread.
enum ServerMsg {
  AddInspector {
    uuid: Uuid,
    inspector_tx: InspectorTx,
    isolate_handle: v8::IsolateHandle,
  },
}

pub enum InspectorMsg {
  WsConnection {
    session_uuid: Uuid,
    ws_tx: WsTx,
  },
  WsIncoming {
    session_uuid: Uuid,
    msg: ws::Message,
  },
}

type ServerMsgTx = mpsc::UnboundedSender<ServerMsg>;
type ServerMsgRx = mpsc::UnboundedReceiver<ServerMsg>;
type WsTx = futures::stream::SplitSink<ws::WebSocket, ws::Message>;
/// Owned by the web socket server
type InspectorTx = mpsc::UnboundedSender<InspectorMsg>;
/// Owned by the isolate/worker
type InspectorRx = mpsc::UnboundedReceiver<InspectorMsg>;

/// Stored in a UUID hashmap, used by WS server. Clonable.
#[derive(Clone)]
struct InspectorInfo {
  inspector_tx: InspectorTx,
  isolate_handle: v8::IsolateHandle,
}

/// Owned by GlobalState.
pub struct InspectorServer {
  address: SocketAddrV4,
  thread_handle: Option<std::thread::JoinHandle<()>>,
  server_msg_tx: Option<ServerMsgTx>,
}

impl InspectorServer {
  /// Starts a DevTools Inspector server on port 127.0.0.1:9229
  pub fn default() -> Self {
    let address = "127.0.0.1:9229".parse::<SocketAddrV4>().unwrap();
    let (server_msg_tx, server_msg_rx) = mpsc::unbounded_channel::<ServerMsg>();
    let thread_handle = std::thread::spawn(move || {
      crate::tokio_util::run_basic(server(address, server_msg_rx));
    });
    Self {
      address,
      thread_handle: Some(thread_handle),
      server_msg_tx: Some(server_msg_tx),
    }
  }

  /// Each worker/isolate to be debugged should call this exactly one.
  /// Called from worker's thread
  pub fn add_inspector(
    &self,
    isolate: &mut deno_core::Isolate,
  ) -> Box<DenoInspector> {
    let deno_core::Isolate {
      v8_isolate,
      global_context,
      ..
    } = isolate;
    let isolate_handle = v8_isolate.as_mut().unwrap().thread_safe_handle();
    let mut hs = v8::HandleScope::new(v8_isolate.as_mut().unwrap());
    let scope = hs.enter();
    let context = global_context.get(scope).unwrap();

    let server_msg_tx = self.server_msg_tx.as_ref().unwrap().clone();
    let address = self.address;
    let (inspector_tx, inspector_rx) =
      mpsc::unbounded_channel::<InspectorMsg>();
    let uuid = Uuid::new_v4();

    let inspector =
      crate::inspector::DenoInspector::new(scope, context, inspector_rx);

    eprintln!(
      "Debugger listening on {}",
      websocket_debugger_url(address, &uuid)
    );

    server_msg_tx
      .send(ServerMsg::AddInspector {
        uuid,
        inspector_tx,
        isolate_handle,
      })
      .unwrap_or_else(|_| {
        panic!("sending message to inspector server thread failed");
      });

    inspector
  }
}

impl Drop for InspectorServer {
  fn drop(&mut self) {
    self.server_msg_tx.take();
    self.thread_handle.take().unwrap().join().unwrap();
    panic!("TODO: this drop is never called");
  }
}

fn websocket_debugger_url(address: SocketAddrV4, uuid: &Uuid) -> String {
  format!("ws://{}:{}/ws/{}", address.ip(), address.port(), uuid)
}

async fn server(address: SocketAddrV4, mut server_msg_rx: ServerMsgRx) {
  let inspector_map = HashMap::<Uuid, InspectorInfo>::new();
  let inspector_map = Arc::new(std::sync::Mutex::new(inspector_map));

  let inspector_map_ = inspector_map.clone();
  let msg_handler = async move {
    while let Some(msg) = server_msg_rx.next().await {
      match msg {
        ServerMsg::AddInspector {
          uuid,
          inspector_tx,
          isolate_handle,
        } => {
          let existing = inspector_map_.lock().unwrap().insert(
            uuid,
            InspectorInfo {
              inspector_tx,
              isolate_handle,
            },
          );
          if existing.is_some() {
            panic!("UUID already in map");
          }
        }
      };
    }
  };

  let inspector_map_ = inspector_map.clone();
  let websocket = warp::path("ws")
    .and(warp::path::param())
    .and(warp::ws())
    .map(move |uuid: String, ws: warp::ws::Ws| {
      let inspector_map__ = inspector_map_.clone();
      ws.on_upgrade(move |socket| async move {
        let inspector_info = {
          if let Ok(uuid) = Uuid::parse_str(&uuid) {
            let g = inspector_map__.lock().unwrap();
            if let Some(inspector_info) = g.get(&uuid) {
              println!("ws connection {}", uuid);
              inspector_info.clone()
            } else {
              return;
            }
          } else {
            return;
          }
        };

        // send a message back so register_worker can return...
        let (ws_tx, mut ws_rx) = socket.split();

        // Not to be confused with the WS's uuid...
        let session_uuid = Uuid::new_v4();

        inspector_info
          .inspector_tx
          .send(InspectorMsg::WsConnection {
            ws_tx,
            session_uuid,
          })
          .unwrap_or_else(|_| {
            panic!("sending message to inspector_tx failed");
          });

        while let Some(Ok(msg)) = ws_rx.next().await {
          inspector_info
            .inspector_tx
            .send(InspectorMsg::WsIncoming { msg, session_uuid })
            .unwrap_or_else(|_| {
              panic!("sending message to inspector_tx failed");
            });
        }
      })
    });

  let inspector_map_ = inspector_map.clone();
  let json_list =
    warp::path("json")
      .map(move || {
        let g = inspector_map_.lock().unwrap();
        let json_values: Vec<serde_json::Value> = g.iter().map(|(uuid, _)| {
          let url = websocket_debugger_url(address, uuid);
          json!({
            "description": "deno",
            "devtoolsFrontendUrl": format!("chrome-devtools://devtools/bundled/js_app.html?experiments=true&v8only=true&ws={}", url),
            "faviconUrl": "https://deno.land/favicon.ico",
            "id": uuid.to_string(),
            "title": format!("deno[{}]", std::process::id()),
            "type": "deno",
            "url": "file://",
            "webSocketDebuggerUrl": url,
          })
        }).collect();
        warp::reply::json(&json!(json_values))
      });

  let version = warp::path!("json" / "version").map(|| {
    warp::reply::json(&json!({
      "Browser": format!("Deno/{}", crate::version::DENO),
      // TODO upgrade to 1.3? https://chromedevtools.github.io/devtools-protocol/
      "Protocol-Version": "1.1",
      "V8-Version": crate::version::v8(),
    }))
  });

  let routes = websocket.or(version).or(json_list);
  let web_handler = warp::serve(routes).bind(address);

  futures::future::join(msg_handler, web_handler).await;
}

#[repr(C)]
pub struct DenoInspector {
  client: v8::inspector::V8InspectorClientBase,
  inspector: v8::UniqueRef<v8::inspector::V8Inspector>,
  terminated: bool,
  sessions: HashMap<Uuid, Box<DenoInspectorSession>>,
  inspector_rx: InspectorRx,
  waiting_for_resume: bool,
  waiting_for_frontend: bool,
}

impl DenoInspector {
  pub fn new<P>(
    scope: &mut P,
    context: v8::Local<v8::Context>,
    inspector_rx: InspectorRx,
  ) -> Box<Self>
  where
    P: v8::InIsolate,
  {
    let mut deno_inspector = new_box_with(|address| Self {
      client: v8::inspector::V8InspectorClientBase::new::<Self>(),
      // TODO(piscisaureus): V8Inspector::create() should require that
      // the 'client' argument cannot move.
      inspector: v8::inspector::V8Inspector::create(scope, unsafe {
        &mut *address
      }),
      terminated: false,
      sessions: HashMap::new(),
      inspector_rx,
      waiting_for_resume: false,
      waiting_for_frontend: false,
    });

    let empty_view = v8::inspector::StringView::empty();
    deno_inspector.inspector.context_created(
      context,
      CONTEXT_GROUP_ID,
      &empty_view,
    );

    deno_inspector
  }

  pub fn connect(&mut self, session_uuid: Uuid, ws_tx: WsTx) {
    let session = DenoInspectorSession::new(&mut self.inspector, ws_tx);
    self.sessions.insert(session_uuid, session);
  }

  fn has_connected_sessions(&self) -> bool {
    !self.sessions.is_empty()
  }

  fn should_run_message_loop(&self) -> bool {
    if self.waiting_for_frontend {
      true
    } else if self.waiting_for_resume {
      self.has_connected_sessions()
    } else {
      false
    }
  }
}

impl v8::inspector::V8InspectorClientImpl for DenoInspector {
  fn base(&self) -> &v8::inspector::V8InspectorClientBase {
    &self.client
  }

  fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
    &mut self.client
  }

  fn run_message_loop_on_pause(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, CONTEXT_GROUP_ID);
    eprintln!("TODO run_message_loop_on_pause");
    // how to get context?
    // TODO self.poll(context);
  }

  fn quit_message_loop_on_pause(&mut self) {
    self.waiting_for_resume = false;
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, CONTEXT_GROUP_ID);
    self.waiting_for_frontend = true;
  }
}

/// DenoInspector implements a Future so that it can poll for incoming messages
/// from the WebSocket server. Since a Worker ownes a DenoInspector, and because
/// a Worker is a Future too, Worker::poll will call this.
impl Future for DenoInspector {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let deno_inspector = self.get_mut();

    // if !deno_inspector.should_run_message_loop() {
    //   return Poll::Ready(());
    // }

    match deno_inspector.inspector_rx.poll_recv(cx) {
      Poll::Pending => Poll::Pending,
      Poll::Ready(Some(msg)) => {
        match msg {
          InspectorMsg::WsConnection {
            session_uuid,
            ws_tx,
          } => {
            deno_inspector.connect(session_uuid, ws_tx);
            println!("Got new ws connection in DenoInspector {}", session_uuid);
          }
          InspectorMsg::WsIncoming { session_uuid, msg } => {
            println!(">>> {}", msg.to_str().unwrap());
            if let Some(deno_session) =
              deno_inspector.sessions.get_mut(&session_uuid)
            {
              deno_session.dispatch_protocol_message(msg)
            } else {
              eprintln!("Unknown session {}. msg {:?}", session_uuid, msg);
            }
          }
        };
        Poll::Ready(())
      }
      Poll::Ready(None) => Poll::Ready(()),
    }
  }
}

/// sub-class of v8::inspector::Channel
pub struct DenoInspectorSession {
  channel: v8::inspector::ChannelBase,
  session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
  ws_tx: WsTx,
}

impl DenoInspectorSession {
  pub fn new(
    inspector: &mut v8::inspector::V8Inspector,
    ws_tx: WsTx,
  ) -> Box<Self> {
    new_box_with(|address| {
      let empty_view = v8::inspector::StringView::empty();
      Self {
        channel: v8::inspector::ChannelBase::new::<Self>(),
        session: inspector.connect(
          CONTEXT_GROUP_ID,
          // Todo(piscisaureus): V8Inspector::connect() should require that
          // the 'channel'  argument cannot move.
          unsafe { &mut *address },
          &empty_view,
        ),
        ws_tx,
      }
    })
  }

  pub fn dispatch_protocol_message(&mut self, ws_msg: ws::Message) {
    let bytes = ws_msg.as_bytes();
    let string_view = v8::inspector::StringView::from(bytes);
    self.session.dispatch_protocol_message(&string_view);
  }
}

impl v8::inspector::ChannelImpl for DenoInspectorSession {
  fn base(&self) -> &v8::inspector::ChannelBase {
    &self.channel
  }

  fn base_mut(&mut self) -> &mut v8::inspector::ChannelBase {
    &mut self.channel
  }

  fn send_response(
    &mut self,
    _call_id: i32,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    let ws_msg = v8_to_ws_msg(message);
    let r = futures::executor::block_on(self.ws_tx.send(ws_msg));
    assert!(r.is_ok());
  }

  fn send_notification(
    &mut self,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    let ws_msg = v8_to_ws_msg(message);
    let r = futures::executor::block_on(self.ws_tx.send(ws_msg));
    assert!(r.is_ok());
  }

  fn flush_protocol_notifications(&mut self) {
    todo!()
  }
}

// TODO impl From or Into
fn v8_to_ws_msg(
  message: v8::UniquePtr<v8::inspector::StringBuffer>,
) -> ws::Message {
  let mut x = message.unwrap();
  let s = x.string().to_string();
  eprintln!("<<< {}", s);
  ws::Message::text(s)
}

fn new_box_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  unsafe { ptr::write(p, new_fn(p)) };
  unsafe { Box::from_raw(p) }
}
