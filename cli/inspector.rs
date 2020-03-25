#![allow(unused_variables)]
#![allow(dead_code)]

use deno_core::v8;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::net::SocketAddrV4;
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex;
use uuid::Uuid;
use warp;
use warp::Filter;

const CONTEXT_GROUP_ID: i32 = 1;

// TODO Currently the value in this map is just (), but we will fill that in
// later with more stuff. Probably channels.
type InspectorMap = Arc<Mutex<HashMap<Uuid, ()>>>;

/// Owned by GlobalState
pub struct InspectorServer {
  address: SocketAddrV4,
  thread_handle: Option<std::thread::JoinHandle<()>>,
  inspector_map: InspectorMap,
}

impl InspectorServer {
  pub fn new() -> Self {
    Self {
      address: "127.0.0.1:9229".parse::<SocketAddrV4>().unwrap(),
      thread_handle: None,
      inspector_map: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  // TODO this should probably be done in impl Drop, but it seems we're leaking
  // GlobalState and so it can't be done there...
  pub fn exit(&mut self) {
    if let Some(thread_handle) = self.thread_handle.take() {
      thread_handle.join().unwrap();
    }
  }

  /// Each worker/isolate to be debugged should call this exactly one.
  pub fn register(&mut self) -> Uuid {
    let uuid = Uuid::new_v4();

    let inspector_map_ = self.inspector_map.clone();
    let mut inspector_map = self.inspector_map.lock().unwrap();
    inspector_map.insert(uuid.clone(), ());

    // Don't start the InspectorServer thread until we have one UUID registered.
    if inspector_map.len() == 1 {
      let address = self.address;
      self.thread_handle = Some(std::thread::spawn(move || {
        println!("Open chrome://inspect/");
        crate::tokio_util::run_basic(server(address, inspector_map_));
      }));
    }

    eprintln!(
      "Debugger listening on {}",
      websocket_debugger_url(self.address, &uuid)
    );

    uuid
  }
}

fn websocket_debugger_url(address: SocketAddrV4, uuid: &Uuid) -> String {
  format!("ws://{}", websocket_debugger_url2(address, uuid))
}

/// Same thing but without "ws://" prefix
fn websocket_debugger_url2(address: SocketAddrV4, uuid: &Uuid) -> String {
  format!("{}:{}/ws/{}", address.ip(), address.port(), uuid)
}

async fn server(address: SocketAddrV4, inspector_map: InspectorMap) -> () {
  let websocket = warp::path("ws")
    .and(warp::path::param())
    .and(warp::ws())
    .map(move |param: String, ws: warp::ws::Ws| {
      println!("ws connection {}", param);
      ws.on_upgrade(move |_socket| async {
        // send a message back so register_worker can return...
        todo!()
      })
    });

  let json_list =
    warp::path("json")
      .map(move || {
        let im = inspector_map.lock().unwrap();
        let json_values: Vec<serde_json::Value> = im.iter().map(|(uuid, _)| {
          let url = websocket_debugger_url(address, uuid);
          let url2 = websocket_debugger_url2(address, uuid);
          json!({
            "description": "deno",
            "devtoolsFrontendUrl": format!("chrome-devtools://devtools/bundled/js_app.html?experiments=true&v8only=true&ws={}", url2),
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
  warp::serve(routes).bind(address).await;
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
pub struct DenoInspector {
  client: v8::inspector::V8InspectorClientBase,
  inspector: v8::UniqueRef<v8::inspector::V8Inspector>,
  terminated: bool,
  sessions: HashMap<usize, Box<DenoInspectorSession>>,
  next_session_id: usize,
}

impl DenoInspector {
  pub fn new<P>(scope: &mut P, context: v8::Local<v8::Context>) -> Box<Self>
  where
    P: v8::InIsolate,
  {
    let mut deno_inspector = new_box_with(|address| Self {
      client: v8::inspector::V8InspectorClientBase::new::<Self>(),
      // Todo(piscisaureus): V8Inspector::create() should require that
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

fn new_box_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  unsafe { ptr::write(p, new_fn(p)) };
  unsafe { Box::from_raw(p) }
}
