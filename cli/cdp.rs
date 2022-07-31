// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <https://chromedevtools.github.io/devtools-protocol/tot/>
use deno_core::serde_json;
use deno_core::serde_json::Value;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-awaitPromise>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwaitPromiseArgs {
  pub promise_object_id: RemoteObjectId,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub return_by_value: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub generate_preview: Option<bool>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-awaitPromise>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwaitPromiseResponse {
  pub result: RemoteObject,
  pub exception_details: Option<ExceptionDetails>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-callFunctionOn>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFunctionOnArgs {
  pub function_declaration: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub object_id: Option<RemoteObjectId>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub arguments: Option<Vec<CallArgument>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub silent: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub return_by_value: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub generate_preview: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub user_gesture: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub await_promise: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub execution_context_id: Option<ExecutionContextId>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub object_group: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub throw_on_side_effect: Option<bool>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-callFunctionOn>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFunctionOnResponse {
  pub result: RemoteObject,
  pub exception_details: Option<ExceptionDetails>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-compileScript>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileScriptArgs {
  pub expression: String,
  #[serde(rename = "sourceURL")]
  pub source_url: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub execution_context_id: Option<ExecutionContextId>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-compileScript>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileScriptResponse {
  pub script_id: Option<ScriptId>,
  pub exception_details: Option<ExceptionDetails>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-evaluate>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateArgs {
  pub expression: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub object_group: Option<String>,
  #[serde(
    rename = "includeCommandLineAPI",
    skip_serializing_if = "Option::is_none"
  )]
  pub include_command_line_api: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub silent: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub context_id: Option<ExecutionContextId>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub return_by_value: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub generate_preview: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub user_gesture: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub await_promise: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub throw_on_side_effect: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub timeout: Option<TimeDelta>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub disable_breaks: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub repl_mode: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[serde(rename = "allowUnsafeEvalBlockedByCSP")]
  pub allow_unsafe_eval_blocked_by_csp: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub unique_context_id: Option<String>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-evaluate>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateResponse {
  pub result: RemoteObject,
  pub exception_details: Option<ExceptionDetails>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-getProperties>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPropertiesArgs {
  pub object_id: RemoteObjectId,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub own_properties: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub accessor_properties_only: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub generate_preview: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub non_indexed_properties_only: Option<bool>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-getProperties>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPropertiesResponse {
  pub result: Vec<PropertyDescriptor>,
  pub internal_properties: Option<Vec<InternalPropertyDescriptor>>,
  pub private_properties: Option<Vec<PrivatePropertyDescriptor>>,
  pub exception_details: Option<ExceptionDetails>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-globalLexicalScopeNames>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalLexicalScopeNamesArgs {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub execution_context_id: Option<ExecutionContextId>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-globalLexicalScopeNames>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalLexicalScopeNamesResponse {
  pub names: Vec<String>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-queryObjects>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryObjectsArgs {
  pub prototype_object_id: RemoteObjectId,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub object_group: Option<String>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-queryObjects>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryObjectsResponse {
  pub objects: RemoteObject,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-releaseObject>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseObjectArgs {
  pub object_id: RemoteObjectId,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-releaseObjectGroup>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseObjectGroupArgs {
  pub object_group: String,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-runScript>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunScriptArgs {
  pub script_id: ScriptId,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub execution_context_id: Option<ExecutionContextId>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub object_group: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub silent: Option<bool>,
  #[serde(
    rename = "includeCommandLineAPI",
    skip_serializing_if = "Option::is_none"
  )]
  pub include_command_line_api: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub return_by_value: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub generate_preview: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub await_promise: Option<bool>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-runScript>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunScriptResponse {
  pub result: RemoteObject,
  pub exception_details: Option<ExceptionDetails>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-setAsyncCallStackDepth>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAsyncCallStackDepthArgs {
  pub max_depth: u64,
}

// types

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-RemoteObject>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteObject {
  #[serde(rename = "type")]
  pub kind: String,
  pub subtype: Option<String>,
  pub class_name: Option<String>,
  #[serde(default, deserialize_with = "deserialize_some")]
  pub value: Option<Value>,
  pub unserializable_value: Option<UnserializableValue>,
  pub description: Option<String>,
  pub object_id: Option<RemoteObjectId>,
  pub preview: Option<ObjectPreview>,
  pub custom_preview: Option<CustomPreview>,
}

// Any value that is present is considered Some value, including null.
// ref: https://github.com/serde-rs/serde/issues/984#issuecomment-314143738
fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
  T: Deserialize<'de>,
  D: Deserializer<'de>,
{
  Deserialize::deserialize(deserializer).map(Some)
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-ObjectPreview>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPreview {
  #[serde(rename = "type")]
  pub kind: String,
  pub subtype: Option<String>,
  pub description: Option<String>,
  pub overflow: bool,
  pub properties: Vec<PropertyPreview>,
  pub entries: Option<Vec<EntryPreview>>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-PropertyPreview>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPreview {
  pub name: String,
  #[serde(rename = "type")]
  pub kind: String,
  pub value: Option<String>,
  pub value_preview: Option<ObjectPreview>,
  pub subtype: Option<String>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-EntryPreview>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPreview {
  pub key: Option<ObjectPreview>,
  pub value: ObjectPreview,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-CustomPreview>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomPreview {
  pub header: String,
  pub body_getter_id: RemoteObjectId,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-ExceptionDetails>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionDetails {
  pub exception_id: u64,
  pub text: String,
  pub line_number: u64,
  pub column_number: u64,
  pub script_id: Option<ScriptId>,
  pub url: Option<String>,
  pub stack_trace: Option<StackTrace>,
  pub exception: Option<RemoteObject>,
  pub execution_context_id: Option<ExecutionContextId>,
  pub exception_meta_data: Option<serde_json::Map<String, Value>>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-StackTrace>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTrace {
  pub description: Option<String>,
  pub call_frames: Vec<CallFrame>,
  pub parent: Option<Box<StackTrace>>,
  pub parent_id: Option<StackTraceId>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-CallFrame>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFrame {
  pub function_name: String,
  pub script_id: ScriptId,
  pub url: String,
  pub line_number: u64,
  pub column_number: u64,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-StackTraceId>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceId {
  pub id: String,
  pub debugger_id: Option<UniqueDebuggerId>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-CallArgument>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallArgument {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub value: Option<Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub unserializable_value: Option<UnserializableValue>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub object_id: Option<RemoteObjectId>,
}

impl From<&RemoteObject> for CallArgument {
  fn from(obj: &RemoteObject) -> Self {
    Self {
      value: obj.value.clone(),
      unserializable_value: obj.unserializable_value.clone(),
      object_id: obj.object_id.clone(),
    }
  }
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-InternalPropertyDescriptor>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyDescriptor {
  pub name: String,
  pub value: Option<RemoteObject>,
  pub writable: Option<bool>,
  pub get: Option<RemoteObject>,
  pub set: Option<RemoteObject>,
  pub configurable: bool,
  pub enumerable: bool,
  pub was_thrown: Option<bool>,
  pub is_own: Option<bool>,
  pub symbol: Option<RemoteObject>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-InternalPropertyDescriptor>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternalPropertyDescriptor {
  pub name: String,
  pub value: Option<RemoteObject>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-PrivatePropertyDescriptor>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivatePropertyDescriptor {
  pub name: String,
  pub value: Option<RemoteObject>,
  pub get: Option<RemoteObject>,
  pub set: Option<RemoteObject>,
}

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-RemoteObjectId>
pub type RemoteObjectId = String;

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-ExecutionContextId>
pub type ExecutionContextId = u64;

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-ScriptId>
pub type ScriptId = String;

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-TimeDelta>
pub type TimeDelta = u64;

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-UnserializableValue>
pub type UnserializableValue = String;

/// <https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#type-UniqueDebuggerId>
pub type UniqueDebuggerId = String;
