#![allow(dead_code)]

use deno_core::v8;
use futures;
use futures::StreamExt;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::net::SocketAddrV4;
use std::ptr;
use std::sync::Arc;

use tokio;
use tokio::sync::mpsc;
use uuid::Uuid;
use warp;
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

type ServerMsgTx = mpsc::UnboundedSender<ServerMsg>;
type ServerMsgRx = mpsc::UnboundedReceiver<ServerMsg>;

pub enum InspectorMsg {
  WsConnection {
    ws_tx: futures::stream::SplitSink<
      warp::filters::ws::WebSocket,
      warp::filters::ws::Message,
    >,
  },
}

/// Owned by the web socket server
type InspectorTx = mpsc::UnboundedSender<InspectorMsg>;
/// Owned by the isolate/worker
type InspectorRx = mpsc::UnboundedReceiver<InspectorMsg>;

// Stored in a UUID hashmap, used by WS server
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
  pub fn new() -> Self {
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

async fn server(address: SocketAddrV4, mut server_msg_rx: ServerMsgRx) -> () {
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
          inspector_map_
            .lock()
            .unwrap()
            .insert(
              uuid,
              InspectorInfo {
                inspector_tx,
                isolate_handle,
              },
            )
            .map(|_| panic!("UUID already in map"));
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

        inspector_info
          .inspector_tx
          .send(InspectorMsg::WsConnection { ws_tx })
          .unwrap_or_else(|_| {
            panic!("sending message to inspector_tx failed");
          });

        while let Some(Ok(msg)) = ws_rx.next().await {
          println!("m: {:?}", msg);
        }
      })
    });

  let inspector_map_ = inspector_map.clone();
  let address_ = address.clone();
  let json_list =
    warp::path("json")
      .map(move || {
        let json_values: Vec<serde_json::Value> = inspector_map_.lock().unwrap().iter().map(|(uuid, _)| {
          let url = websocket_debugger_url(address_, uuid);
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
  sessions: HashMap<usize, Box<DenoInspectorSession>>,
  next_session_id: usize,
}

impl DenoInspector {
  pub fn new<P>(
    scope: &mut P,
    context: v8::Local<v8::Context>,
    mut inspector_rx: InspectorRx,
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
      next_session_id: 1,
    });

    let empty_view = v8::inspector::StringView::empty();
    deno_inspector.inspector.context_created(
      context,
      CONTEXT_GROUP_ID,
      &empty_view,
    );

    tokio::spawn(async move {
      while let Some(msg) = inspector_rx.next().await {
        match msg {
          InspectorMsg::WsConnection { .. } => {
            println!("Got new ws connection in DenoInspector");
            // TODO need to create a new session.
          }
        }
      }
    });

    deno_inspector
  }

  pub fn connect(&mut self) {
    let id = self.next_session_id;
    self.next_session_id += 1;
    let session = DenoInspectorSession::new(&mut self.inspector);
    self.sessions.insert(id, session);
  }
}

impl v8::inspector::V8InspectorClientImpl for DenoInspector {
  fn base(&self) -> &v8::inspector::V8InspectorClientBase {
    &self.client
  }

  fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
    &mut self.client
  }

  fn run_message_loop_on_pause(&mut self, _context_group_id: i32) {
    // while !self.terminated {
    // self.deno_isolate.inspector_block_recv();
    // }
    todo!()
  }

  fn quit_message_loop_on_pause(&mut self) {
    todo!()
  }

  fn run_if_waiting_for_debugger(&mut self, _context_group_id: i32) {
    todo!()
  }
}

/// sub-class of v8::inspector::Channel
pub struct DenoInspectorSession {
  channel: v8::inspector::ChannelBase,
  session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
}

impl DenoInspectorSession {
  pub fn new(inspector: &mut v8::inspector::V8Inspector) -> Box<Self> {
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
      }
    })
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
    _message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    // deno_isolate.inspector_message_cb(message)
    todo!()
  }

  fn send_notification(
    &mut self,
    _message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    // deno_isolate.inspector_message_cb(message)
    todo!()
  }

  fn flush_protocol_notifications(&mut self) {
    // pass
    todo!()
  }
}

fn new_box_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  unsafe { ptr::write(p, new_fn(p)) };
  unsafe { Box::from_raw(p) }
}
