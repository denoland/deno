use deno_core::serde_json::Value;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RpcNotification {
  pub method: String,
  pub params: Value,
}

#[derive(Debug, Deserialize)]
pub struct SetScriptSourceReturnObject {
  pub status: Status,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptParsed {
  pub script_id: String,
  pub url: String,
}

#[derive(Debug, Deserialize)]
pub enum Status {
  Ok,
  CompileError,
  BlockedByActiveGenerator,
  BlockedByActiveFunction,
  BlockedByTopLevelEsModuleChange,
}

impl Status {
  pub(crate) fn explain(&self) -> &'static str {
    match self {
      Status::Ok => "OK",
      Status::CompileError => "compile error",
      Status::BlockedByActiveGenerator => "blocked by active generator",
      Status::BlockedByActiveFunction => "blocked by active function",
      Status::BlockedByTopLevelEsModuleChange => {
        "blocked by top-level ES module change"
      }
    }
  }

  pub(crate) fn should_retry(&self) -> bool {
    match self {
      Status::Ok => false,
      Status::CompileError => false,
      Status::BlockedByActiveGenerator => true,
      Status::BlockedByActiveFunction => true,
      Status::BlockedByTopLevelEsModuleChange => false,
    }
  }
}
