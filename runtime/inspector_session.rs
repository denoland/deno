// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

//! The documentation for the inspector API is sparse, but these are helpful:
//! https://chromedevtools.github.io/devtools-protocol/
//! https://hyperandroid.com/2020/02/12/v8-inspector-from-an-embedder-standpoint/

use crate::deno_inspector::new_box_with;
use crate::deno_inspector::DenoInspectorBase;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::channel::oneshot;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::v8;
use std::collections::HashMap;
use std::ops::Deref;
use std::ops::DerefMut;

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

  pub fn new(inspector_ptr: *mut DenoInspectorBase) -> Box<Self> {
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
