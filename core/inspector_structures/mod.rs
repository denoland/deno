pub mod runtime;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "method", content = "params")]
pub enum Methods {
  RuntimeAwaitPromise(runtime::AwaitPromiseArgs),
  RuntimeCallFunctionOn(runtime::CallFunctionOnArgs),
  RuntimeCompileScript(runtime::CompileScriptArgs),
  RuntimeDisable,
  RuntimeDiscardConsoleEntries,
  RuntimeEnable,
  RuntimeEvaluate(runtime::EvaluateArgs),
  RuntimeGetProperties(runtime::GetPropertiesArgs),
  RuntimeGlobalLexicalScopeNames(runtime::GlobalLexicalScopeNamesArgs),
  RuntimeQueryObjects(runtime::QueryObjectsArgs),
  RuntimeReleaseObject(runtime::ReleaseObjectArgs),
  RuntimeReleaseObjectGroup(runtime::ReleaseObjectGroupArgs),
  RuntimeRunIfWaitingForDebugger,
  RuntimeRunScript(runtime::RunScriptArgs),
  RuntimeSetAsyncCallStackDepth(runtime::SetAsyncCallStackDepthArgs),
}
