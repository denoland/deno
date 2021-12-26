// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json::Value;

// /* *****************
//  * SHELL MESSAGES
//  * *****************/
// Shell Request Message Types
// "execute_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#execute
// "inspect_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
// "complete_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
// "history_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
// "is_complete_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
// "comm_info_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
// "kernel_info_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info

// Shell Reply Message Types
// "execute_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-results
// "inspect_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
// "complete_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
// "history_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
// "is_complete_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
// "comm_info_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
// "kernel_info_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info

#[derive(Debug, Serialize, Deserialize)]
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
  // Side Effects
  Status(KernelStatusContent),
  Stream(StreamContent),
  ExecuteInput(ExecuteInputContent),
  ExecuteResult(ExecuteResultContent),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RequestMetadata {
  Empty,
  Unknown(Value),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ReplyMetadata {
  Empty,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execute
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteRequestContent {
  pub code: String,
  pub silent: bool,
  pub store_history: bool,
  pub user_expressions: Value,
  pub allow_stdin: bool,
  pub stop_on_error: bool,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-results
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteReplyContent {
  pub status: String,
  pub execution_count: u32,
  // TODO: "None" gets translated to "null"
  // payload: Option<Vec<String>>,
  // user_expressions: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
pub struct InspectRequestContent {
  pub code: String,
  pub cursor_pos: u32,
  pub detail_level: u8, // 0 = Low, 1 = High
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#introspection
pub struct InspectReplyContent {
  pub status: String,
  pub found: bool,
  pub data: Option<Value>,
  pub metadata: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
pub struct CompleteRequestContent {
  pub code: String,
  pub cursor_pos: u32,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#completion
pub struct CompleteReplyContent {
  pub status: String,
  pub matches: Option<Value>,
  pub cursor_start: u32,
  pub cursor_end: u32,
  pub metadata: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
pub struct HistoryRequestContent {
  pub output: bool,
  pub raw: bool,
  pub hist_access_type: String, // "range" | "tail" | "search"
  pub session: u32,
  pub start: u32,
  pub stop: u32,
  pub n: u32,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#history
pub struct HistoryReplyContent {
  pub status: String,
  pub history: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
pub struct CodeCompleteRequestContent {
  pub code: String,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-completeness
pub struct CodeCompleteReplyContent {
  pub status: String, // "complete" | "incomplete" | "invalid" | "unknown"
  pub indent: String,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
pub struct CommInfoRequestContent {
  pub target_name: String,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-info
pub struct CommInfoReplyContent {
  pub status: String,
  pub comms: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
// pub struct KernelInfoRequest {} // empty

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
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

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
#[derive(Debug, Serialize, Deserialize)]
pub struct KernelLanguageInfo {
  pub name: String,
  pub version: String,
  pub mimetype: String,
  pub file_extension: String,
  // TODO: "None" gets translated to "null"
  // pygments_lexer: Option<String>,
  // codemirror_mode: Option<Value>,
  // nbconvert_exporter: Option<String>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
#[derive(Debug, Serialize, Deserialize)]
pub struct KernelHelpLink {
  pub text: String,
  pub url: String,
}

/* *****************
 * CONTROL MESSAGES
 * *****************/

// Control Request Message Types
// "shutdown_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
// "interrupt_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
// "debug_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request

// Control Reply Message Types
// "shutdown_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
// "interrupt_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
// "debug_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
pub struct ShutdownRequestContent {
  pub restart: bool,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-shutdown
pub struct ShutdownReplyContent {
  pub status: String,
  pub restart: bool,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
// pub struct InterruptRequestContent {} // empty

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-interrupt
pub struct InterruptReplyContent {
  pub status: String,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request
// pub struct DebugRequestContent {} // See Debug Adapter Protocol (DAP) 1.39 or later

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request
// pub struct DebugReplyContent {} // See Debug Adapter Protocol (DAP) 1.39 or later

/* *****************
 * IOPUB MESSAGES
 * *****************/

// Io Pub Message Types
// "stream" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#streams-stdout-stderr-etc
// "display_data" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#display-data
// "update_display_data" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#update-display-data
// "execute_input" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-inputs
// "execute_result" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#id6
// "error" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-errors
// "status" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-status
// "clear_output" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#clear-output
// "debug_event" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-event
// "comm_open" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#opening-a-comm
// "comm_msg" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-messages
// "comm_close" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#tearing-down-comms

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#request-reply
pub struct ErrorStatusContent {
  pub status: String, // "error"
  pub ename: String,
  pub evalue: String,
  pub traceback: Vec<String>,
  pub execution_count: Option<u32>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#request-reply
// #[derive(Debug, Serialize, Deserialize)]
// pub struct StatusContent {
//   status: String, // "ok" | "abort"
// }

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#streams-stdout-stderr-etc
#[derive(Debug, Serialize, Deserialize)]
pub struct StreamContent {
  pub name: String, // "stdout" | "stderr"
  pub text: String,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#display-data
pub struct DisplayDataContent {
  pub data: Value,
  pub metadata: Option<Value>,
  pub transient: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#update-display-data
// pub struct UpdateDisplayDataContent {} // same as DisplayDataContent

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-inputs
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteInputContent {
  pub code: String,
  pub execution_count: u32,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#id6
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteResultContent {
  pub execution_count: u32,
  pub data: Option<Value>,
  pub metadata: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#execution-errors
pub struct ErrorContent {
  pub payload: Option<Vec<String>>,
  pub user_expressions: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-status
#[derive(Debug, Serialize, Deserialize)]
pub struct KernelStatusContent {
  pub execution_state: String, // "busy" | "idle" | "starting"
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#clear-output
pub struct ClearOutputContent {
  pub wait: bool,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-event
// pub struct DebugEventContent {} // see Event message from the Debug Adapter Protocol (DAP)

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#opening-a-comm
pub struct CommOpenMessage {
  pub comm_id: uuid::Uuid,
  pub target_name: String,
  pub data: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-messages
pub struct CommMsgMessage {
  pub comm_id: uuid::Uuid,
  pub data: Option<Value>,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#comm-messages
pub struct CommCloseMessage {
  pub comm_id: uuid::Uuid,
  pub data: Option<Value>,
}

/* *****************
 * STDIN MESSAGES
 * *****************/
// Stdin Request Message Types
// "input_request" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel

// Stdin Reply Message Types
// "input_reply" // https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel
pub struct InputRequestContent {
  pub prompt: String,
  pub password: bool,
}

// https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel
pub struct InputReplyContent {
  pub value: String,
}
