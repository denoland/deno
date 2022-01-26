use deno_core::serde_json;
use deno_core::serde_json::Value;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwaitPromiseArgs {
  pub promise_object_id: RemoteObjectId,
  pub return_by_value: Option<bool>,
  pub generate_preview: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwaitPromiseResponse {
  pub result: RemoteObject,
  pub exception_details: Option<ExceptionDetails>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFunctionOnArgs {
  pub function_declaration: String,
  pub object_id: Option<RemoteObjectId>,
  pub arguments: Option<Vec<CallArgument>>,
  pub silent: Option<bool>,
  pub return_by_value: Option<bool>,
  pub generate_preview: Option<bool>,
  pub user_gesture: Option<bool>,
  pub await_promise: Option<bool>,
  pub execution_context_id: Option<ExecutionContextId>,
  pub object_group: Option<String>,
  pub throw_on_side_effect: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFunctionOnResponse {
  pub result: RemoteObject,
  pub exception_details: Option<ExceptionDetails>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileScriptArgs {
  pub expression: String,
  #[serde(rename = "sourceURL")]
  pub source_url: String,
  pub execution_context_id: Option<ExecutionContextId>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileScriptResponse {
  pub script_id: Option<ScriptId>,
  pub exception_details: Option<ExceptionDetails>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateArgs {
  pub expression: String,
  pub object_group: Option<String>,
  #[serde(rename = "includeCommandLineAPI")]
  pub include_command_line_api: Option<bool>,
  pub silent: Option<bool>,
  pub context_id: Option<ExecutionContextId>,
  pub return_by_value: Option<bool>,
  pub generate_preview: Option<bool>,
  pub user_gesture: Option<bool>,
  pub await_promise: Option<bool>,
  pub throw_on_side_effect: Option<bool>,
  pub timeout: Option<TimeDelta>,
  pub disable_breaks: Option<bool>,
  pub repl_mode: Option<bool>,
  #[serde(rename = "allowUnsafeEvalBlockedByCSP")]
  pub allow_unsafe_eval_blocked_by_csp: Option<bool>,
  pub unique_context_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateResponse {
  pub result: RemoteObject,
  pub exception_details: Option<ExceptionDetails>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPropertiesArgs {
  pub object_id: RemoteObjectId,
  pub own_properties: Option<bool>,
  pub accessor_properties_only: Option<bool>,
  pub generate_preview: Option<bool>,
  pub non_indexed_properties_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPropertiesResponse {
  pub result: Vec<PropertyDescriptor>,
  pub internal_properties: Vec<InternalPropertyDescriptor>,
  pub private_properties: Vec<PrivatePropertyDescriptor>,
  pub exception_details: Option<ExceptionDetails>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalLexicalScopeNamesArgs {
  pub execution_context_id: Option<ExecutionContextId>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalLexicalScopeNamesResponse {
  pub names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryObjectsArgs {
  pub prototype_object_id: RemoteObjectId,
  pub object_group: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryObjectsResponse {
  pub objects: RemoteObject,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseObjectArgs {
  pub object_id: RemoteObjectId,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseObjectGroupArgs {
  pub object_group: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunScriptArgs {
  pub script_id: ScriptId,
  pub execution_context_id: Option<ExecutionContextId>,
  pub object_group: Option<String>,
  pub silent: Option<bool>,
  #[serde(rename = "includeCommandLineAPI")]
  pub include_command_line_api: Option<bool>,
  pub return_by_value: Option<bool>,
  pub generate_preview: Option<bool>,
  pub await_promise: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunScriptResponse {
  pub result: RemoteObject,
  pub exception_details: Option<ExceptionDetails>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAsyncCallStackDepthArgs {
  pub max_depth: u64,
}

// types

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteObjectType {
  Object,
  Function,
  Undefined,
  String,
  Number,
  Boolean,
  Symbol,
  Bigint,
}

impl std::fmt::Display for RemoteObjectType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&format!("{:?}", self).to_lowercase())
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteObjectSubType {
  Array,
  Null,
  Node,
  Regexp,
  Date,
  Map,
  Set,
  Weakmap,
  Weakset,
  Iterator,
  Generator,
  Error,
  Proxy,
  Promise,
  Typedarray,
  Arraybuffer,
  Dataview,
  Webassemblymemory,
  Wasmvalue,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteObject {
  #[serde(rename = "type")]
  pub kind: RemoteObjectType,
  pub subtype: Option<RemoteObjectSubType>,
  pub class_name: Option<String>,
  pub value: Option<Value>,
  pub unserializable_value: Option<UnserializableValue>,
  pub description: Option<String>,
  pub object_id: Option<RemoteObjectId>,
  pub preview: Option<ObjectPreview>,
  pub custom_preview: Option<CustomPreview>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPreview {
  #[serde(rename = "type")]
  pub kind: RemoteObjectType,
  pub subtype: Option<RemoteObjectSubType>,
  pub description: Option<String>,
  pub overflow: bool,
  pub properties: Vec<PropertyPreview>,
  pub entries: Option<Vec<EntryPreview>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPreview {
  pub name: String,
  #[serde(rename = "type")]
  pub kind: PropertyPreviewType,
  pub value: Option<String>,
  pub value_preview: Option<ObjectPreview>,
  pub subtype: Option<RemoteObjectSubType>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PropertyPreviewType {
  Object,
  Function,
  Undefined,
  String,
  Number,
  Boolean,
  Symbol,
  Accessor,
  Bigint,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPreview {
  pub key: Option<ObjectPreview>,
  pub value: ObjectPreview,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomPreview {
  pub header: String,
  pub body_getter_id: RemoteObjectId,
}

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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTrace {
  pub description: Option<String>,
  pub call_frames: Vec<CallFrame>,
  pub parent: Option<Box<StackTrace>>,
  pub parent_id: Option<StackTraceId>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFrame {
  pub function_name: String,
  pub script_id: ScriptId,
  pub url: String,
  pub line_number: u64,
  pub column_number: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceId {
  pub id: String,
  pub debugger_id: Option<UniqueDebuggerId>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallArgument {
  pub value: Option<Value>,
  pub unserializable_value: Option<UnserializableValue>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternalPropertyDescriptor {
  pub name: String,
  pub value: Option<RemoteObject>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivatePropertyDescriptor {
  pub name: String,
  pub value: Option<RemoteObject>,
  pub get: Option<RemoteObject>,
  pub set: Option<RemoteObject>,
}

pub type RemoteObjectId = String;
pub type ExecutionContextId = u64;
pub type ScriptId = String;
pub type TimeDelta = u64;
pub type UnserializableValue = String;
pub type UniqueDebuggerId = String;
