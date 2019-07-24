use crate::version::DENO;
use futures::sync::oneshot::{spawn, SpawnHandle};
use futures::{Future, Sink, Stream};
use serde_json::Value;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use warp::ws::{Message, WebSocket};
use warp::Filter;

static UUID: &str = "97690037-256e-4e27-add0-915ca5421e2f";
// TODO(mtharrison): Make these configurable with flags, defaulting to below values
static HOST: &str = "127.0.0.1";
static PORT: &str = "9888";

lazy_static! {
    #[derive(Serialize)]
    pub static ref RESPONSE_JSON: Value = json!([{
      "description": "deno",
      "devtoolsFrontendUrl": format!("chrome-devtools://devtools/bundled/js_app.html?experiments=true&v8only=true&ws={}:{}/websocket", HOST, PORT),
      "devtoolsFrontendUrlCompat": format!("chrome-devtools://devtools/bundled/inspector.html?experiments=true&v8only=true&ws={}:{}/websocket", HOST, PORT),
      "faviconUrl": "https://www.deno-play.app/images/deno.svg",
      "id": UUID,
      "title": format!("deno[{}]", std::process::id()),
      "type": "deno",
      "url": "file://",
      "webSocketDebuggerUrl": format!("ws://{}:{}/websocket", HOST, PORT)
    }]);

    #[derive(Serialize)]
    pub static ref RESPONSE_VERSION: Value = json!({
      "Browser": format!("Deno/{}", DENO),
      "Protocol-Version": "1.1",
      "webSocketDebuggerUrl": format!("ws://{}:{}/{}", HOST, PORT, UUID)
    });
}

pub struct Inspector {
  // sharable handle to channels passed to isolate
  pub handle: deno::InspectorHandle,
  // sending/receving messages from isolate
  inbound_tx: Arc<Mutex<Sender<String>>>,
  outbound_rx: Arc<Mutex<Receiver<String>>>,
  // signals readiness of inspector
  // TODO(mtharrison): Is a condvar more appropriate?
  ready_tx: Arc<Mutex<Sender<()>>>,
  ready_rx: Arc<Mutex<Receiver<()>>>,
  server_spawn_handle: Option<SpawnHandle<(), ()>>,
  // TODO(mtharrison): Maybe use an atomic bool instead?
  connected: Arc<Mutex<bool>>,
}

impl Inspector {
  pub fn new() -> Self {
    let (inbound_tx, inbound_rx) = channel::<String>();
    let (outbound_tx, outbound_rx) = channel::<String>();
    let (ready_tx, ready_rx) = channel::<()>();

    Inspector {
      handle: deno::InspectorHandle::new(outbound_tx, inbound_rx),
      inbound_tx: Arc::new(Mutex::new(inbound_tx)),
      outbound_rx: Arc::new(Mutex::new(outbound_rx)),
      ready_rx: Arc::new(Mutex::new(ready_rx)),
      ready_tx: Arc::new(Mutex::new(ready_tx)),
      server_spawn_handle: None,
      connected: Arc::new(Mutex::new(false)),
    }
  }

  pub fn serve(&self) -> impl Future<Item = (), Error = ()> {
    let inbound_tx = self.inbound_tx.clone();
    let outbound_rx = self.outbound_rx.clone();
    let ready_tx = self.ready_tx.clone();
    let connected = self.connected.clone();

    let websocket =
      warp::path("websocket")
        .and(warp::ws2())
        .map(move |ws: warp::ws::Ws2| {
          // TODO(mtharrison): is there a cleaner way to do this? I couldn't find one

          let sender = inbound_tx.clone();
          let receiver = outbound_rx.clone();
          let ready_tx = ready_tx.clone();
          let connected = connected.clone();

          ws.on_upgrade(move |socket| {
            on_connection(socket, sender, receiver, ready_tx, connected)
          })
        });

    let json = warp::path("json").map(|| warp::reply::json(&*RESPONSE_JSON));

    let version =
      path!("json" / "version").map(|| warp::reply::json(&*RESPONSE_VERSION));

    let routes = websocket.or(version).or(json);

    let endpoint = format!("{}:{}", HOST, PORT);
    let addr = endpoint.parse::<std::net::SocketAddrV4>().unwrap();

    warp::serve(routes).bind(addr)
  }

  pub fn start(&mut self, wait: bool) {
    self.server_spawn_handle = Some(spawn(
      self.serve(),
      &tokio_executor::DefaultExecutor::current(),
    ));

    println!("Debugger listening on ws://{}:{}/{}", HOST, PORT, UUID);

    if wait {
      self
        .ready_rx
        .lock()
        .unwrap()
        .recv()
        .expect("Error waiting for inspector server to start.");
      // TODO(mtharrison): This is to allow v8 to ingest some inspector messages - find a more reliable way
      std::thread::sleep(std::time::Duration::from_secs(1));
      println!("Inspector frontend connected.");
    }
  }

  pub fn stop(&mut self) {}
}

fn on_connection(
  ws: WebSocket,
  sender: Arc<Mutex<Sender<String>>>,
  receiver: Arc<Mutex<Receiver<String>>>,
  ready_tx: Arc<Mutex<Sender<()>>>,
  connected: Arc<Mutex<bool>>,
) -> impl Future<Item = (), Error = ()> {
  let (mut user_ws_tx, user_ws_rx) = ws.split();

  let fut_rx = user_ws_rx
    .for_each(move |msg| {
      let msg_str = msg.to_str().unwrap();
      sender
        .lock()
        .unwrap()
        .send(msg_str.to_owned())
        .unwrap_or_else(|err| println!("Err: {}", err));
      Ok(())
    }).map_err(|_| {});

  // TODO(mtharrison): This is a mess. There must be a better way to do this - maybe use async channels or wrap them with a stream?

  std::thread::spawn(move || {
    let receiver = receiver.lock().unwrap();
    loop {
      let received = receiver.recv();
      if let Ok(msg) = received {
        let _ = ready_tx.lock().unwrap().send(());
        *connected.lock().unwrap() = true;
        user_ws_tx = user_ws_tx.send(Message::text(msg)).wait().unwrap();
      }
    }
  });

  fut_rx
}

impl Drop for Inspector {
  fn drop(&mut self) {
    if *self.connected.lock().unwrap() == true {
      println!("Waiting for debugger to disconnect...");
    }
  }
}
