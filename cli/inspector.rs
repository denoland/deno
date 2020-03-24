#![allow(unused_variables)]
#![allow(dead_code)]

use deno_core::v8;
use std::collections::HashMap;
use std::net::SocketAddrV4;
use warp;
use warp::Filter;

const CONTEXT_GROUP_ID: i32 = 1;

/// Owned by GlobalState
pub struct InspectorServer {
  thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl InspectorServer {
  pub fn new() -> Self {
    let thread_handle = std::thread::spawn(move || {
      println!("debug");
      println!("debug before run_basic");
      crate::tokio_util::run_basic(server());
      println!("debug after run_basic");
    });
    Self {
      // control_sender: sender,
      thread_handle: Some(thread_handle),
    }
  }

  // TODO this should probably be done in impl Drop, but it seems we're leaking
  // GlobalState and so it can't be done there...
  pub fn exit(&mut self) {
    self.thread_handle.take().unwrap().join().unwrap();
  }

  pub async fn register_worker() {
    todo!()
  }
}

async fn server() -> () {
  let websocket =
    warp::path("websocket")
      .and(warp::ws())
      .map(move |ws: warp::ws::Ws| {
        ws.on_upgrade(move |_socket| async {
          // here

          // send a message back so register_worker can return...

          todo!()
          // let client = Client::new(state, socket);
          // client.on_connection()
        })
      });

  // todo(matt): Make this unique per run (https://github.com/denoland/deno/pull/2696#discussion_r309282566)
  let uuid = "97690037-256e-4e27-add0-915ca5421e2f";

  let address = "127.0.0.1:9229".parse::<SocketAddrV4>().unwrap();
  let ip = format!("{}", address.ip());
  let port = address.port();

  let response_json = json!([{
    "description": "deno",
    "devtoolsFrontendUrl": format!("chrome-devtools://devtools/bundled/js_app.html?experiments=true&v8only=true&ws={}:{}/websocket", ip, port),
    "devtoolsFrontendUrlCompat": format!("chrome-devtools://devtools/bundled/inspector.html?experiments=true&v8only=true&ws={}:{}/websocket", ip, port),
    "faviconUrl": "https://www.deno-play.app/images/deno.svg",
    "id": uuid,
    "title": format!("deno[{}]", std::process::id()),
    "type": "deno",
    "url": "file://",
    "webSocketDebuggerUrl": format!("ws://{}:{}/websocket", ip, port)
  }]);

  let response_version = json!({
    "Browser": format!("Deno/{}", crate::version::DENO),
    "Protocol-Version": "1.1",
    "webSocketDebuggerUrl": format!("ws://{}:{}/{}", ip, port, uuid)
  });

  let json = warp::path("json").map(move || warp::reply::json(&response_json));

  let version = warp::path!("json" / "version")
    .map(move || warp::reply::json(&response_version));

  let routes = websocket.or(version).or(json);
  warp::serve(routes).bind(address).await;
}

/// sub-class of v8::inspector::Channel
pub struct DenoInspectorSession {
  base: v8::inspector::ChannelBase,
  session: Option<v8::UniqueRef<v8::inspector::V8InspectorSession>>,
}

impl DenoInspectorSession {
  pub fn new(inspector: &mut v8::inspector::V8Inspector) -> Self {
    let mut x = Self {
      base: v8::inspector::ChannelBase::new::<Self>(),
      session: None,
    };
    let empty_view = v8::inspector::StringView::empty();
    let session = inspector.connect(CONTEXT_GROUP_ID, &mut x, &empty_view);

    x.session = Some(session);

    x
  }
}

impl v8::inspector::ChannelImpl for DenoInspectorSession {
  fn base(&self) -> &v8::inspector::ChannelBase {
    &self.base
  }

  fn base_mut(&mut self) -> &mut v8::inspector::ChannelBase {
    &mut self.base
  }

  fn send_response(
    &mut self,
    call_id: i32,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    // deno_isolate.inspector_message_cb(message)
    todo!()
  }

  fn send_notification(
    &mut self,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    // deno_isolate.inspector_message_cb(message)
    todo!()
  }

  fn flush_protocol_notifications(&mut self) {
    // pass
    todo!()
  }
}

#[repr(C)]
pub struct DenoInspectorClient {
  base: v8::inspector::V8InspectorClientBase,

  // Note this is the same as NodeInspectorClient::client_
  inspector: Option<v8::UniqueRef<v8::inspector::V8Inspector>>,

  terminated: bool,

  // Note this is the same as NodeInspectorClient::channels_
  sessions: HashMap<usize, DenoInspectorSession>,

  next_session_id: usize,
}

impl DenoInspectorClient {
  pub fn new<P>(scope: &mut P, context: v8::Local<v8::Context>) -> Self
  where
    P: v8::InIsolate,
  {
    let mut client = Self {
      base: v8::inspector::V8InspectorClientBase::new::<Self>(),
      inspector: None,
      terminated: false,
      sessions: HashMap::new(),
      next_session_id: 1,
    };

    let mut inspector = v8::inspector::V8Inspector::create(scope, &mut client);

    let empty_view = v8::inspector::StringView::empty();
    let buffer = v8::inspector::StringBuffer::create(&empty_view).unwrap();

    let state = b"";
    let state_view = v8::inspector::StringView::from(&state[..]);

    inspector.context_created(context, CONTEXT_GROUP_ID, &state_view);

    client.inspector = Some(inspector);
    client
  }

  pub fn connect(&mut self) {
    let id = self.next_session_id;
    self.next_session_id += 1;
    let session = DenoInspectorSession::new(self.inspector.as_mut().unwrap());
    self.sessions.insert(id, session);
  }
}

impl v8::inspector::V8InspectorClientImpl for DenoInspectorClient {
  fn base(&self) -> &v8::inspector::V8InspectorClientBase {
    &self.base
  }

  fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
    &mut self.base
  }

  fn run_message_loop_on_pause(&mut self, context_group_id: i32) {
    // while !self.terminated {
    // self.deno_isolate.inspector_block_recv();
    // }
    todo!()
  }

  fn quit_message_loop_on_pause(&mut self) {
    todo!()
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: i32) {
    todo!()
  }
}
