//! This module implements a browser-side Chrome DevTools Protocol (CDP) server.
//! It is a layer on-top of the v8 inspector protocol. It dispatches methods
//! from incoming sessions to either the v8 inspector, or handlers for other
//! DevTools commands (for example Network.enable). The module additionally
//! faciliates sending events over the DevTools protocol back to the client.

use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;

use crate::error::AnyError;
use crate::futures;
use crate::futures::channel::mpsc::UnboundedReceiver;
use crate::futures::channel::mpsc::UnboundedSender;
use crate::futures::try_join;
use crate::futures::StreamExt;
use crate::inspector::InspectorSessionProxy;
use rusty_v8 as v8;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

/// The DevToolsAgent is the link between ops and the DevTools. One agent exists
/// per isolate. An agent is shared between the inspector thread and the isolate
/// thread, so it must be thread-safe.
///
/// The agent is responsible for:
///  - Keeping a list of all the active DevTools sessions
///  - Providing a way for ops to dispatch events to these sessions
///  - Providing a way for ops to share data with the CDP request handlers
#[derive(Clone, Default)]
pub struct DevToolsAgent {
  pub sessions: Arc<Mutex<Vec<DevToolsSession>>>,

  // TODO(lucacasonato): add a GothamState

  // This is temporary. Instead `DevToolsAgent` should get its own GothamState
  // where stuff like this can be stored.
  pub request_bodies: Arc<Mutex<HashMap<Uuid, Vec<u8>>>>,
}

fn domain_from_method(method: &str) -> Option<&str> {
  method.split_once(".").map(|(domain, _)| domain)
}

impl DevToolsAgent {
  /// Check if any session has the given domain enabled.
  pub fn has_subscribers_for_domain(&self, domain: &str) -> bool {
    let sessions = self.sessions.lock().unwrap();
    for session in &*sessions {
      if session.is_domain_enabled(domain) {
        eprintln!("has_subscribers_for_domain {} true", domain);
        return true;
      }
    }
    eprintln!("has_subscribers_for_domain {} false", domain);
    false
  }

  /// Send an event to all connected DevTools sessions.
  pub fn notify_all(&self, method: &str, params: serde_json::Value) {
    let sessions = self.sessions.lock().unwrap();
    for session in &*sessions {
      let _ = session.send(CdpMessage::Event {
        method: method.to_owned(),
        params: params.clone(),
      });
    }
  }

  /// Send an event to all connected DevTools sessions where the method's
  /// domain is enabled.
  pub fn notify_subscribers(&self, method: &str, params: serde_json::Value) {
    let domain =
      domain_from_method(method).expect("method is missing a domain");
    let sessions = self.sessions.lock().unwrap();
    for session in &*sessions {
      if session.is_domain_enabled(domain) {
        let _ = session.send(CdpMessage::Event {
          method: method.to_owned(),
          params: params.clone(),
        });
      }
    }
  }
}

/// Messages in the Chrome DevTools Protocol format. This represents both
/// incoming and outgoing messages.
#[derive(Deserialize, Serialize, Debug)]
#[serde(untagged)]
pub enum CdpMessage {
  // NOTE: The order of structs in this enum is important. Serde trys to
  // deserializes the structs in the order they appear in this enum!
  #[serde(rename_all = "camelCase")]
  Request {
    id: i32,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
    #[serde(default)]
    session_id: String,
  },
  #[serde(rename_all = "camelCase")]
  Event {
    method: String,
    #[serde(default)]
    params: serde_json::Value,
  },
  #[serde(rename_all = "camelCase")]
  ResponseOk { id: i32, result: serde_json::Value },
  #[serde(rename_all = "camelCase")]
  ResponseErr { id: i32, error: CdpError },
}

/// An error in the Chrome DevTools Protocol format.
/// Common error codes can be found at:
/// https://source.chromium.org/chromium/chromium/src/+/main:third_party/inspector_protocol/crdtp/dispatch.h;drc=5e82cf877e87ed14142e0cd82fd8408084f3a43b
#[derive(Deserialize, Serialize, Debug)]
pub struct CdpError {
  code: i32,
  message: String,
}

impl CdpMessage {
  /// Return the method name for a CDP message if it is a request or an event.
  pub fn method(&self) -> Option<&str> {
    match self {
      CdpMessage::Request { method, .. } => Some(method),
      CdpMessage::Event { method, .. } => Some(method),
      CdpMessage::ResponseOk { .. } => None,
      CdpMessage::ResponseErr { .. } => None,
    }
  }

  /// Check if V8 inspector can handle the message.
  pub fn can_dispatch_to_v8(&self) -> bool {
    self
      .method()
      .map(|m| {
        let string_view = v8::inspector::StringView::from(m.as_bytes());
        v8::inspector::V8InspectorSession::can_dispatch_method(string_view)
      })
      .unwrap_or(false)
  }
}

// This is a temporary solution to dispatching until we have a dynamic
// dispatch system, just like ops.
fn handle_message(
  session: &DevToolsSession,
  method: &str,
  params: serde_json::Value,
) -> Result<serde_json::Value, CdpError> {
  match method {
    "Network.enable" => {
      eprintln!("enabled network");
      session.enable_domain("Network");
      Ok(json!({}))
    }
    "Network.disable" => {
      eprintln!("disabled network");
      session.disable_domain("Network");
      Ok(json!({}))
    }
    "Network.getResponseBody" => {
      #[derive(Deserialize)]
      #[serde(rename_all = "camelCase")]
      struct Params {
        request_id: Uuid,
      }
      let params: Params = serde_json::from_value(params).unwrap();
      let request_bodies = session.agent.request_bodies.lock().unwrap();
      let body = request_bodies.get(&params.request_id).unwrap();
      Ok(json!({
        "base64Encoded": true,
        "body": base64::encode(body)
      }))
    }
    _ => {
      let message = format!("Method '{}' not found", method);
      Err(CdpError {
        // Error codes: https://source.chromium.org/chromium/chromium/src/+/main:third_party/inspector_protocol/crdtp/dispatch.h;drc=5e82cf877e87ed14142e0cd82fd8408084f3a43b
        code: -32601,
        message,
      })
    }
  }
}

/// The DevTools session represents a single Chrome DevTools session on a
/// specific isolate. There can be multiple sessions in one isolate.
///
/// The session is responsible for:
///   - Maintaining the domain enablement state across the lifetime of a session
///   - Sharing per session state between multiple CDP requests, and event
///     dispatches from ops.
#[derive(Clone)]
pub struct DevToolsSession {
  transport_tx: UnboundedSender<String>,
  v8_tx: UnboundedSender<Result<Vec<u8>, AnyError>>,

  /// The agent this session is associated with.
  pub agent: DevToolsAgent,

  /// The domains that are enabled for this session.
  enabled_domains: Arc<Mutex<HashSet<String>>>,
}

impl DevToolsSession {
  /// Enable a domain for this session.
  pub fn enable_domain(&self, domain: &str) {
    let mut domains = self.enabled_domains.lock().unwrap();
    domains.insert(domain.to_owned());
  }

  /// Disable a domain for this session.
  pub fn disable_domain(&self, domain: &str) {
    let mut domains = self.enabled_domains.lock().unwrap();
    domains.remove(domain);
  }

  /// Check if a domain is enabled for this session.
  pub fn is_domain_enabled(&self, domain: &str) -> bool {
    let domains = self.enabled_domains.lock().unwrap();
    domains.contains(domain)
  }
}

impl DevToolsSession {
  /// Start a new DevTools session.
  ///
  /// Returns a tuple of channels that represent the external transport (for
  /// example via websockets), a InspectorSessionProxy that faciliates
  /// communication with the V8 inspector, and a future that needs to be
  /// polled to drive the session forward.
  pub fn start(
    agent: DevToolsAgent,
  ) -> (
    UnboundedReceiver<String>,
    UnboundedSender<Vec<u8>>,
    InspectorSessionProxy,
    impl Future<Output = Result<(), AnyError>> + Send,
  ) {
    let (transport_inbound_tx, mut transport_inbound_rx) =
      futures::channel::mpsc::unbounded::<Vec<u8>>();
    let (transport_outbound_tx, transport_outbound_rx) =
      futures::channel::mpsc::unbounded::<String>();

    let (v8_inbound_tx, v8_inbound_rx) = futures::channel::mpsc::unbounded();
    let (v8_outbound_tx, mut v8_outbound_rx) =
      futures::channel::mpsc::unbounded();

    let session_proxy = InspectorSessionProxy {
      rx: v8_inbound_rx,
      tx: v8_outbound_tx,
    };

    let session = DevToolsSession {
      transport_tx: transport_outbound_tx.clone(),
      v8_tx: v8_inbound_tx,
      agent: agent.clone(),
      enabled_domains: Default::default(),
    };
    let session_ = session.clone();

    let incoming = async move {
      while let Some(data) = transport_inbound_rx.next().await {
        let msg: CdpMessage = serde_json::from_slice(&data)?;
        session_.dispatch(msg)?;
      }
      Ok::<_, AnyError>(())
    };

    let from_v8 = async move {
      while let Some((_, msg)) = v8_outbound_rx.next().await {
        if transport_outbound_tx.unbounded_send(msg).is_err() {
          break;
        }
      }
      Ok::<_, AnyError>(())
    };

    let fut = async move {
      try_join!(incoming, from_v8)?;
      Ok(())
    };

    {
      let mut sessions = agent.sessions.lock().unwrap();
      sessions.push(session);
    }

    (
      transport_outbound_rx,
      transport_inbound_tx,
      session_proxy,
      fut,
    )
  }

  fn dispatch(&self, msg: CdpMessage) -> Result<(), AnyError> {
    if msg.can_dispatch_to_v8() {
      return self.dispatch_to_v8(msg);
    }

    let (id, method, params) = match msg {
      CdpMessage::Request {
        id, method, params, ..
      } => (id, method, params),
      _ => {
        eprintln!("Unexpected message type");
        return Ok(());
      }
    };

    // TODO(lucacasonato): At this point we need to dispatch requests based on
    // the method name. This should be done similarly to how we dispatch ops.
    // Unlike ops, CDP requests are always asynchonous, so they will always be
    // backed by a future. DevToolsAgent should have it's own GothamState to
    // be able to pass data between ops and CDP requests (for example fetch
    // response bodies).
    //
    // The signature for a request handler should be:
    // async fn<P: Deserialize, R: Serialize>(session: DevToolsSession, params: P) -> Result<R, CdpError>

    // TODO(lucacasonato): Temporary handler while the above is implemented.
    let res = handle_message(self, &method, params);

    let msg = match res {
      Ok(result) => CdpMessage::ResponseOk { id, result },
      Err(error) => CdpMessage::ResponseErr { id, error },
    };
    self.send(msg)?;

    Ok(())
  }

  fn send(&self, msg: CdpMessage) -> Result<(), AnyError> {
    let data = serde_json::to_string(&msg)?;
    let _ = self.transport_tx.unbounded_send(data);
    Ok(())
  }

  fn dispatch_to_v8(&self, msg: CdpMessage) -> Result<(), AnyError> {
    let data = serde_json::to_vec(&msg)?;
    self.v8_tx.unbounded_send(Ok(data)).unwrap();
    Ok(())
  }
}
