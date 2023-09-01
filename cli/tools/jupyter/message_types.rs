// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// TODO(bartlomieju): remove this lint supression
#![allow(dead_code)]

use data_encoding::HEXLOWER;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use ring::hmac;
use zeromq::ZmqMessage;

const DELIMITER: &str = "<IDS|MSG>";

#[derive(Clone, Debug)]
pub struct RequestMessage {
  pub header: MessageHeader,
  pub parent_header: Option<()>,
  pub metadata: RequestMetadata,
  pub content: RequestContent,
}

impl RequestMessage {
  pub fn new(
    header: MessageHeader,
    metadata: RequestMetadata,
    content: RequestContent,
  ) -> Self {
    Self {
      header,
      parent_header: None,
      metadata,
      content,
    }
  }
}

impl TryFrom<ZmqMessage> for RequestMessage {
  type Error = AnyError;

  fn try_from(zmq_msg: ZmqMessage) -> Result<Self, AnyError> {
    // TODO(apowers313) make all unwraps recoverable errors
    let header_bytes = zmq_msg.get(2).unwrap();
    let _metadata_bytes = zmq_msg.get(4).unwrap();
    let content_bytes = zmq_msg.get(5).unwrap();

    let header: MessageHeader = serde_json::from_slice(header_bytes).unwrap();
    let msg_type = header.msg_type.clone();

    // TODO(apowers313) refactor to an unwrap function to handles unwrapping based on different protocol versions
    let mc = match msg_type.as_ref() {
      "kernel_info_request" => (RequestMetadata::Empty, RequestContent::Empty),
      "execute_request" => (
        RequestMetadata::Empty,
        RequestContent::Execute(serde_json::from_slice(content_bytes).unwrap()),
      ),
      _ => (RequestMetadata::Empty, RequestContent::Empty),
    };

    let rm = RequestMessage::new(header, mc.0, mc.1);
    log::debug!("<== RECEIVING MSG [{}]: {:#?}", msg_type, rm);

    Ok(rm)
  }
}

#[derive(Debug)]
pub struct ReplyMessage {
  pub header: MessageHeader,
  pub parent_header: MessageHeader,
  pub metadata: ReplyMetadata,
  pub content: ReplyContent,
}

impl ReplyMessage {
  pub fn new(
    comm_ctx: &CommContext,
    msg_type: &str,
    metadata: ReplyMetadata,
    content: ReplyContent,
  ) -> Self {
    Self {
      header: MessageHeader::new(
        msg_type,
        comm_ctx.message.header.session.clone(),
      ),
      parent_header: comm_ctx.message.header.clone(),
      metadata,
      content,
    }
  }

  pub fn serialize(&self, hmac_key: &hmac::Key) -> ZmqMessage {
    // TODO(apowers313) convert unwrap() to recoverable error
    let header = serde_json::to_string(&self.header).unwrap();
    let parent_header = serde_json::to_string(&self.parent_header).unwrap();
    let _metadata = serde_json::to_string(&self.metadata).unwrap();
    let metadata = match &self.metadata {
      ReplyMetadata::Empty => serde_json::to_string(&json!({})).unwrap(),
    };
    let content = match &self.content {
      ReplyContent::Empty => serde_json::to_string(&json!({})).unwrap(),
      // reply messages
      ReplyContent::KernelInfo(c) => serde_json::to_string(&c).unwrap(),
      ReplyContent::ExecuteReply(c) => serde_json::to_string(&c).unwrap(),
      ReplyContent::ExecuteError(c) => serde_json::to_string(&c).unwrap(),
      // side effects
      ReplyContent::Status(c) => serde_json::to_string(&c).unwrap(),
      ReplyContent::Stream(c) => serde_json::to_string(&c).unwrap(),
      ReplyContent::ExecuteInput(c) => serde_json::to_string(&c).unwrap(),
      ReplyContent::ExecuteResult(c) => serde_json::to_string(&c).unwrap(),
      ReplyContent::Error(c) => serde_json::to_string(&c).unwrap(),
      ReplyContent::DisplayData(c) => serde_json::to_string(&c).unwrap(),
    };

    let hmac =
      hmac_sign(hmac_key, &header, &parent_header, &metadata, &content);

    let mut zmq_msg = ZmqMessage::from(DELIMITER);
    zmq_msg.push_back(hmac.into());
    zmq_msg.push_back(header.into());
    zmq_msg.push_back(parent_header.into());
    zmq_msg.push_back(metadata.into());
    zmq_msg.push_back(content.into());
    log::debug!(
      "==> SENDING MSG [{}]: {:#?}",
      &self.header.msg_type,
      zmq_msg
    );

    zmq_msg
  }
}

// side effects messages sent on IOPub look lik ReplyMessages (for now?)
pub type SideEffectMessage = ReplyMessage;

#[derive(Clone, Debug)]
pub struct CommContext {
  pub message: RequestMessage,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MessageHeader {
  pub msg_id: String,
  pub session: String,
  pub username: String,
  // TODO(apowers313) -- date as an Option is to address a Jupyter bug
  // see also: https://github.com/jupyter/notebook/issues/6257
  #[serde(default = "missing_date")]
  pub date: String,
  pub msg_type: String,
  pub version: String,
}

impl MessageHeader {
  pub fn new(msg_type: &str, session: String) -> Self {
    let now = std::time::SystemTime::now();
    let now: chrono::DateTime<chrono::Utc> = now.into();
    let now = now.to_rfc3339();

    Self {
      msg_id: uuid::Uuid::new_v4().to_string(),
      session,
      // FIXME:
      username: "<TODO>".to_string(),
      date: now,
      msg_type: msg_type.to_string(),
      // TODO: this should be taken from a global,
      version: "5.3".to_string(),
    }
  }
}

fn missing_date() -> String {
  "1970-01-01T00:00:00+00:00".to_string()
}

// /* *****************
//  * SHELL MESSAGES
//  * *****************/
// Shell Request Message Types
// "execute_request" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execute
// "inspect_request" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
// "complete_request" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
// "history_request" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
// "is_complete_request" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
// "comm_info_request" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
// "kernel_info_request" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info

// Shell Reply Message Types
// "execute_reply" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-results
// "inspect_reply" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
// "complete_reply" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
// "history_reply" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
// "is_complete_reply" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
// "comm_info_reply" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
// "kernel_info_reply" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RequestContent {
  Empty,
  Execute(ExecuteRequestContent),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ReplyContent {
  Empty,
  // Reply Messages
  KernelInfo(KernelInfoReplyContent),
  ExecuteReply(ExecuteReplyContent),
  ExecuteError(ExecuteErrorContent),
  // Side Effects
  Status(KernelStatusContent),
  Stream(StreamContent),
  ExecuteInput(ExecuteInputContent),
  ExecuteResult(ExecuteResultContent),
  Error(ErrorContent),
  DisplayData(DisplayDataContent),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RequestMetadata {
  Empty,
  Unknown(Value),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ReplyMetadata {
  Empty,
}

//// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execute
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteRequestContent {
  pub code: String,
  pub silent: bool,
  pub store_history: bool,
  pub user_expressions: Value,
  pub allow_stdin: bool,
  pub stop_on_error: bool,
}

//// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-results
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteReplyContent {
  pub status: String,
  pub execution_count: u32,
  // These two fields are unused
  pub payload: Vec<String>,
  // #[serde(skip_serializing_if = "Option::is_none")]
  pub user_expressions: Value,
}

//// https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
pub struct InspectRequestContent {
  pub code: String,
  pub cursor_pos: u32,
  pub detail_level: u8, // 0 = Low, 1 = High
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
pub struct InspectReplyContent {
  pub status: String,
  pub found: bool,
  pub data: Option<Value>,
  pub metadata: Option<Value>,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
pub struct CompleteRequestContent {
  pub code: String,
  pub cursor_pos: u32,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
pub struct CompleteReplyContent {
  pub status: String,
  pub matches: Option<Value>,
  pub cursor_start: u32,
  pub cursor_end: u32,
  pub metadata: Option<Value>,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
pub struct HistoryRequestContent {
  pub output: bool,
  pub raw: bool,
  pub hist_access_type: String, // "range" | "tail" | "search"
  pub session: u32,
  pub start: u32,
  pub stop: u32,
  pub n: u32,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
pub struct HistoryReplyContent {
  pub status: String,
  pub history: Option<Value>,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
pub struct CodeCompleteRequestContent {
  pub code: String,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
pub struct CodeCompleteReplyContent {
  pub status: String, // "complete" | "incomplete" | "invalid" | "unknown"
  pub indent: String,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
pub struct CommInfoRequestContent {
  pub target_name: String,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
pub struct CommInfoReplyContent {
  pub status: String,
  pub comms: Option<Value>,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
// pub struct KernelInfoRequest {} // empty

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
#[derive(Debug, Serialize, Deserialize)]
pub struct KernelInfoReplyContent {
  pub status: String,
  pub protocol_version: String,
  pub implementation: String,
  pub implementation_version: String,
  pub language_info: KernelLanguageInfo,
  pub banner: String,
  pub debugger: bool,
  pub help_links: Vec<KernelHelpLink>,
}

//// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
///
/// `pygments_lexer`, `codemirror_more` and `nvconvert_exporter` are omitted.
#[derive(Debug, Serialize, Deserialize)]
pub struct KernelLanguageInfo {
  pub name: String,
  pub version: String,
  pub mimetype: String,
  pub file_extension: String,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
#[derive(Debug, Serialize, Deserialize)]
pub struct KernelHelpLink {
  pub text: String,
  pub url: String,
}

/* *****************
 * CONTROL MESSAGES
 * *****************/

// Control Request Message Types
// "shutdown_request" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
// "interrupt_request" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
// "debug_request" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request

// Control Reply Message Types
// "shutdown_reply" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
// "interrupt_reply" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
// "debug_reply" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
pub struct ShutdownRequestContent {
  pub restart: bool,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
pub struct ShutdownReplyContent {
  pub status: String,
  pub restart: bool,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
// pub struct InterruptRequestContent {} // empty

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
pub struct InterruptReplyContent {
  pub status: String,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request
// pub struct DebugRequestContent {} // See Debug Adapter Protocol (DAP) 1.39 or later

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request
// pub struct DebugReplyContent {} // See Debug Adapter Protocol (DAP) 1.39 or later

/* *****************
 * IOPUB MESSAGES
 * *****************/
// Io Pub Message Types
// "stream" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#streams-stdout-stderr-etc
// "display_data" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#display-data
// "update_display_data" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#update-display-data
// "execute_input" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-inputs
// "execute_result" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#id6
// "error" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-errors
// "status" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-status
// "clear_output" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#clear-output
// "debug_event" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-event
// "comm_open" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#opening-a-comm
// "comm_msg" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-messages
// "comm_close" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#tearing-down-comms

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#request-reply
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorContent {
  pub ename: String,
  pub evalue: String,
  pub traceback: Vec<String>,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#request-reply
// #[derive(Debug, Serialize, Deserialize)]
// pub struct StatusContent {
//   status: String, // "ok" | "abort"
// }

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#streams-stdout-stderr-etc
#[derive(Debug, Serialize, Deserialize)]
pub struct StreamContent {
  pub name: String, // "stdout" | "stderr"
  pub text: String,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#display-data
#[derive(Debug, Serialize, Deserialize)]
pub struct DisplayDataContent {
  pub data: Value,
  pub metadata: Value,
  pub transient: Value,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#update-display-data
// pub struct UpdateDisplayDataContent {} // same as DisplayDataContent

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-inputs
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteInputContent {
  pub code: String,
  pub execution_count: u32,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#id6
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteResultContent {
  pub execution_count: u32,
  pub data: Value,
  pub metadata: Value,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-errors
/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-shell-router-dealer-channel
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteErrorContent {
  pub execution_count: u32,
  pub status: String,
  pub payload: Vec<String>,
  pub user_expressions: Value,
  // XXX: the spec says one thing and the ipykernal does another... the following three fields are included by the ipykernel
  pub ename: String,
  pub evalue: String,
  pub traceback: Vec<String>,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-status
#[derive(Debug, Serialize, Deserialize)]
pub struct KernelStatusContent {
  pub execution_state: String, // "busy" | "idle" | "starting"
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#clear-output
pub struct ClearOutputContent {
  pub wait: bool,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-event
// pub struct DebugEventContent {} // see Event message from the Debug Adapter Protocol (DAP)

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#opening-a-comm
pub struct CommOpenMessage {
  pub comm_id: uuid::Uuid,
  pub target_name: String,
  pub data: Option<Value>,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-messages
pub struct CommMsgMessage {
  pub comm_id: uuid::Uuid,
  pub data: Option<Value>,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-messages
pub struct CommCloseMessage {
  pub comm_id: uuid::Uuid,
  pub data: Option<Value>,
}

/* *****************
 * STDIN MESSAGES
 * *****************/
// Stdin Request Message Types
// "input_request" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel

// Stdin Reply Message Types
// "input_reply" /// https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel
pub struct InputRequestContent {
  pub prompt: String,
  pub password: bool,
}

/// https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel
pub struct InputReplyContent {
  pub value: String,
}

fn hmac_sign(
  key: &hmac::Key,
  header: &str,
  parent_header: &str,
  metadata: &str,
  content: &str,
) -> String {
  let mut hmac_ctx = hmac::Context::with_key(key);
  hmac_ctx.update(header.as_bytes());
  hmac_ctx.update(parent_header.as_bytes());
  hmac_ctx.update(metadata.as_bytes());
  hmac_ctx.update(content.as_bytes());
  let tag = hmac_ctx.sign();
  let sig = HEXLOWER.encode(tag.as_ref());
  sig
}

pub fn hmac_verify(
  key: &hmac::Key,
  expected_signature: &[u8],
  header: &[u8],
  parent_header: &[u8],
  metadata: &[u8],
  content: &[u8],
) -> Result<(), AnyError> {
  let mut msg = Vec::<u8>::new();
  msg.extend(header);
  msg.extend(parent_header);
  msg.extend(metadata);
  msg.extend(content);
  let sig = HEXLOWER.decode(expected_signature)?;
  hmac::verify(key, msg.as_ref(), sig.as_ref())?;
  Ok(())
}

#[test]
fn hmac_verify_test() {
  let key_value = "1f5cec86-8eaa942eef7f5a35b51ddcf5";
  let key = hmac::Key::new(hmac::HMAC_SHA256, key_value.as_ref());

  let expected_signature =
    "43a5c45062e0b6bcc59c727f90165ad1d2eb02e1c5317aa25c2c2049d96d3b6a"
      .as_bytes()
      .to_vec();
  let header = "{\"msg_id\":\"c0fd20872c1b4d1c87e7fc814b75c93e_0\",\"msg_type\":\"kernel_info_request\",\"username\":\"ampower\",\"session\":\"c0fd20872c1b4d1c87e7fc814b75c93e\",\"date\":\"2021-12-10T06:20:40.259695Z\",\"version\":\"5.3\"}".as_bytes().to_vec();
  let parent_header = "{}".as_bytes().to_vec();
  let metadata = "{}".as_bytes().to_vec();
  let content = "{}".as_bytes().to_vec();

  let result = hmac_verify(
    &key,
    &expected_signature,
    &header,
    &parent_header,
    &metadata,
    &content,
  );

  assert!(result.is_ok(), "signature validation failed");
}

#[test]
fn hmac_sign_test() {
  let key_value = "1f5cec86-8eaa942eef7f5a35b51ddcf5";
  let key = hmac::Key::new(hmac::HMAC_SHA256, key_value.as_ref());
  let header = "{\"msg_id\":\"c0fd20872c1b4d1c87e7fc814b75c93e_0\",\"msg_type\":\"kernel_info_request\",\"username\":\"ampower\",\"session\":\"c0fd20872c1b4d1c87e7fc814b75c93e\",\"date\":\"2021-12-10T06:20:40.259695Z\",\"version\":\"5.3\"}";
  let parent_header = "{}";
  let metadata = "{}";
  let content = "{}";
  let sig = hmac_sign(&key, header, parent_header, metadata, content);
  assert_eq!(
    sig,
    "43a5c45062e0b6bcc59c727f90165ad1d2eb02e1c5317aa25c2c2049d96d3b6a"
  );
}
