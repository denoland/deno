// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

//! The documentation for the inspector API is sparse, but these are helpful:
//! https://chromedevtools.github.io/devtools-protocol/
//! https://hyperandroid.com/2020/02/12/v8-inspector-from-an-embedder-standpoint/

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::channel::oneshot;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::v8;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ptr;
use std::cell::RefCell;

pub struct DenoInspector {
  v8_inspector_client: v8::inspector::V8InspectorClientBase,
  v8_inspector: v8::UniquePtr<v8::inspector::V8Inspector>,
  flags: RefCell<InspectorFlags>,
}

impl Deref for DenoInspector {
  type Target = v8::inspector::V8Inspector;
  fn deref(&self) -> &Self::Target {
    self.v8_inspector.as_ref().unwrap()
  }
}

impl DerefMut for DenoInspector {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.v8_inspector.as_mut().unwrap()
  }
}

impl v8::inspector::V8InspectorClientImpl for DenoInspector {
  fn base(&self) -> &v8::inspector::V8InspectorClientBase {
    &self.v8_inspector_client
  }

  fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
    &mut self.v8_inspector_client
  }

  fn run_message_loop_on_pause(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, DenoInspector::CONTEXT_GROUP_ID);
    self.flags.borrow_mut().on_pause = true;
  }

  fn quit_message_loop_on_pause(&mut self) {
    self.flags.borrow_mut().on_pause = false;
  }

  fn run_if_waiting_for_debugger(&mut self, context_group_id: i32) {
    assert_eq!(context_group_id, DenoInspector::CONTEXT_GROUP_ID);
    self.flags.borrow_mut().session_handshake_done = true;
  }
}

impl DenoInspector {
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(js_runtime: &mut deno_core::JsRuntime) -> Box<Self> {
    let context = js_runtime.global_context();
    let scope = &mut v8::HandleScope::new(js_runtime.v8_isolate());

    // Create DenoInspector instance.
    let mut self_ = new_box_with(|self_ptr| {
      let v8_inspector_client =
        v8::inspector::V8InspectorClientBase::new::<Self>();

      let flags = InspectorFlags::new();

      Self {
        v8_inspector_client,
        v8_inspector: Default::default(),
        flags,
      }
    });
    self_.v8_inspector =
      v8::inspector::V8Inspector::create(scope, &mut *self_).into();

    // Tell the inspector about the global context.
    let context = v8::Local::new(scope, context);
    let context_name = v8::inspector::StringView::from(&b"global context"[..]);
    self_.context_created(context, Self::CONTEXT_GROUP_ID, context_name);

    self_
  }
}

#[derive(Default)]
struct InspectorFlags {
  waiting_for_session: bool,
  session_handshake_done: bool,
  on_pause: bool,
}

impl InspectorFlags {
  fn new() -> RefCell<Self> {
    let self_ = Self::default();
    RefCell::new(self_)
  }
}

/// A local inspector session that can be used to send and receive protocol messages directly on
/// the same thread as an isolate.
pub struct InspectorSession {
  v8_channel: v8::inspector::ChannelBase,
  v8_session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
  response_tx_map: HashMap<i32, oneshot::Sender<serde_json::Value>>,
  next_message_id: i32,
  notification_queue: Vec<Value>,
}

impl Deref for InspectorSession {
  type Target = v8::inspector::V8InspectorSession;
  fn deref(&self) -> &Self::Target {
    &self.v8_session
  }
}

impl DerefMut for InspectorSession {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.v8_session
  }
}

impl v8::inspector::ChannelImpl for InspectorSession {
  fn base(&self) -> &v8::inspector::ChannelBase {
    &self.v8_channel
  }

  fn base_mut(&mut self) -> &mut v8::inspector::ChannelBase {
    &mut self.v8_channel
  }

  fn send_response(
    &mut self,
    call_id: i32,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    let raw_message = message.unwrap().string().to_string();
    let message: serde_json::Value = match serde_json::from_str(&raw_message) {
      Ok(v) => v,
      Err(error) => match error.classify() {
        serde_json::error::Category::Syntax => json!({
          "id": call_id,
          "result": {
            "result": {
              "type": "error",
              "description": "Unterminated string literal",
              "value": "Unterminated string literal",
            },
            "exceptionDetails": {
              "exceptionId": 0,
              "text": "Unterminated string literal",
              "lineNumber": 0,
              "columnNumber": 0
            },
          },
        }),
        _ => panic!("Could not parse inspector message"),
      },
    };

    self
      .response_tx_map
      .remove(&call_id)
      .unwrap()
      .send(message)
      .unwrap();
  }

  fn send_notification(
    &mut self,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    let raw_message = message.unwrap().string().to_string();
    let message = serde_json::from_str(&raw_message).unwrap();

    self.notification_queue.push(message);
  }

  fn flush_protocol_notifications(&mut self) {}
}

impl InspectorSession {
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(inspector_ptr: *mut DenoInspector) -> Box<Self> {
    new_box_with(move |self_ptr| {
      let v8_channel = v8::inspector::ChannelBase::new::<Self>();
      let v8_session = unsafe { &mut *inspector_ptr }.connect(
        Self::CONTEXT_GROUP_ID,
        unsafe { &mut *self_ptr },
        v8::inspector::StringView::empty(),
      );

      let response_tx_map = HashMap::new();
      let next_message_id = 0;

      let notification_queue = Vec::new();

      Self {
        v8_channel,
        v8_session,
        response_tx_map,
        next_message_id,
        notification_queue,
      }
    })
  }

  pub fn notifications(&mut self) -> Vec<Value> {
    self.notification_queue.split_off(0)
  }

  pub async fn post_message(
    &mut self,
    method: &str,
    params: Option<serde_json::Value>,
  ) -> Result<serde_json::Value, AnyError> {
    let id = self.next_message_id;
    self.next_message_id += 1;

    let (response_tx, response_rx) = oneshot::channel::<serde_json::Value>();
    self.response_tx_map.insert(id, response_tx);

    let message = json!({
        "id": id,
        "method": method,
        "params": params,
    });

    let raw_message = serde_json::to_string(&message).unwrap();
    let raw_message = v8::inspector::StringView::from(raw_message.as_bytes());
    self.v8_session.dispatch_protocol_message(raw_message);

    let response = response_rx.await.unwrap();
    if let Some(error) = response.get("error") {
      return Err(generic_error(error.to_string()));
    }

    let result = response.get("result").unwrap().clone();
    Ok(result)
  }
}

fn new_box_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  unsafe { ptr::write(p, new_fn(p)) };
  unsafe { Box::from_raw(p) }
}
