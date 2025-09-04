// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::OpState;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::serde_v8;
use deno_core::v8;
use deno_error::JsErrorBox;

deno_core::extension!(
  deno_bundle_runtime,
  deps = [
    deno_web
  ],
  ops = [
    op_bundle,
  ],
  objects = [
    PluginExecResultSenderWrapper,
  ],
  esm = [
    "bundle.ts"
  ],
  options = {
    bundle_provider: Option<Arc<dyn BundleProvider>>,
  },
  state = |state, options| {
    if let Some(bundle_provider) = options.bundle_provider {
      state.put(bundle_provider);
    } else {
      state.put::<Arc<dyn BundleProvider>>(Arc::new(()));
    }
  },
);

#[async_trait]
impl BundleProvider for () {
  async fn bundle(
    &self,
    _options: BundleOptions,
    _plugins: Option<Plugins>,
  ) -> Result<BuildResponse, AnyError> {
    Err(deno_core::anyhow::anyhow!(
      "default BundleProvider does not do anything"
    ))
  }
}

#[async_trait]
pub trait BundleProvider: Send + Sync {
  async fn bundle(
    &self,
    options: BundleOptions,
    plugins: Option<Plugins>,
  ) -> Result<BuildResponse, AnyError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleOptions {
  pub entrypoints: Vec<String>,
  #[serde(default)]
  pub output_path: Option<String>,
  #[serde(default)]
  pub output_dir: Option<String>,
  #[serde(default)]
  pub external: Vec<String>,
  #[serde(default)]
  pub format: BundleFormat,
  #[serde(default)]
  pub minify: bool,
  #[serde(default)]
  pub code_splitting: bool,
  #[serde(default = "tru")]
  pub inline_imports: bool,
  #[serde(default)]
  pub packages: PackageHandling,
  #[serde(default)]
  pub sourcemap: Option<SourceMapType>,
  #[serde(default)]
  pub platform: BundlePlatform,
  #[serde(default = "tru")]
  pub write: bool,
}

fn tru() -> bool {
  true
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BundlePlatform {
  Browser,
  #[default]
  Deno,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BundleFormat {
  #[default]
  Esm,
  Cjs,
  Iife,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceMapType {
  #[default]
  Linked,
  Inline,
  External,
}

impl std::fmt::Display for BundleFormat {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      BundleFormat::Esm => write!(f, "esm"),
      BundleFormat::Cjs => write!(f, "cjs"),
      BundleFormat::Iife => write!(f, "iife"),
    }
  }
}

impl std::fmt::Display for SourceMapType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SourceMapType::Linked => write!(f, "linked"),
      SourceMapType::Inline => write!(f, "inline"),
      SourceMapType::External => write!(f, "external"),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageHandling {
  #[default]
  Bundle,
  External,
}

impl std::fmt::Display for PackageHandling {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PackageHandling::Bundle => write!(f, "bundle"),
      PackageHandling::External => write!(f, "external"),
    }
  }
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
  pub text: String,
  pub location: Option<Location>,
  pub notes: Vec<Note>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialMessage {
  pub id: Option<String>,
  pub plugin_name: Option<String>,
  pub text: Option<String>,
  pub location: Option<Location>,
  pub notes: Option<Vec<Note>>,
  pub detail: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildOutputFile {
  pub path: String,
  pub contents: Option<Vec<u8>>,
  pub hash: String,
}
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildResponse {
  pub errors: Vec<Message>,
  pub warnings: Vec<Message>,
  pub output_files: Option<Vec<BuildOutputFile>>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
  pub text: String,
  pub location: Option<Location>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
  pub file: String,
  pub namespace: Option<String>,
  pub line: u32,
  pub column: u32,
  pub length: Option<u32>,
  pub suggestion: Option<String>,
}

fn deserialize_regex<'de, D>(deserializer: D) -> Result<regex::Regex, D::Error>
where
  D: serde::Deserializer<'de>,
{
  use serde::Deserialize;
  let s = String::deserialize(deserializer)?;
  regex::Regex::new(&s).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnResolveOptions {
  #[serde(deserialize_with = "deserialize_regex")]
  pub filter: regex::Regex,
  pub namespace: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnLoadOptions {
  #[serde(deserialize_with = "deserialize_regex")]
  pub filter: regex::Regex,
  pub namespace: Option<String>,
}

#[op2(async)]
#[serde]
pub async fn op_bundle(
  state: Rc<RefCell<OpState>>,
  #[serde] options: BundleOptions,
  #[serde] plugins: Option<Vec<PluginInfo>>,
  #[global] plugin_executor: Option<v8::Global<v8::Function>>,
) -> Result<BuildResponse, JsErrorBox> {
  log::trace!("op_bundle: {:?}", options);
  // eprintln!("op_bundle: {:?}", options);
  let (provider, spawner) = {
    let state = state.borrow();
    let provider = state.borrow::<Arc<dyn BundleProvider>>().clone();
    let spawner = state.borrow::<deno_core::V8TaskSpawner>().clone();
    (provider, spawner)
  };

  let (plugins, plugin_executor_fut) = if let Some(plugin_executor) =
    plugin_executor
    && let Some(plugin_info) = plugins
  {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<PluginRequest>(1024);
    let fut = async move {
      loop {
        log::trace!("op_bundle: rx.recv");
        let Some(request) = rx.recv().await else {
          break;
        };
        log::trace!("op_bundle: rx.recv: {:?}", request);
        let plugin_executor = plugin_executor.clone();
        spawner.spawn(move |scope| {
          let tc = &mut v8::TryCatch::new(scope);
          let args = request.to_args(tc).unwrap();
          let executor = v8::Local::new(tc, plugin_executor);
          let undef = v8::undefined(tc).into();
          log::trace!("op_bundle: executor.call");
          let _res = executor.call(tc, undef, &args).unwrap();
        });
      }
    }
    .boxed_local();
    (
      Some(Plugins {
        sender: tx,
        info: plugin_info,
      }),
      fut,
    )
  } else {
    (None, async move {}.boxed_local())
  };

  log::trace!("op_bundle: provider.bundle");
  let (bundle_result, _) =
    tokio::join!(provider.bundle(options, plugins), plugin_executor_fut);
  log::trace!("op_bundle: bundle_result: {:?}", bundle_result);

  bundle_result.map_err(|e| JsErrorBox::generic(e.to_string()))
}

// Plugin plumbing types and ops
use std::convert::Infallible;
use tokio::sync::oneshot;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
  pub name: String,
  pub id: u32,
  pub on_start: bool,
  pub on_end: bool,
  pub on_resolve: Option<OnResolveOptions>,
  pub on_load: Option<OnLoadOptions>,
  pub on_dispose: bool,
}

#[derive(Debug)]
pub enum PluginRequest {
  OnStart {
    plugin_ids: Vec<u32>,
    args: Vec<serde_json::Value>,
    result: oneshot::Sender<PluginResult<PluginOnStartResult>>,
  },
  OnResolve {
    plugin_ids: Vec<u32>,
    args: Vec<serde_json::Value>,
    result: oneshot::Sender<PluginResult<Option<PluginOnResolveResult>>>,
  },
  OnLoad {
    plugin_ids: Vec<u32>,
    args: Vec<serde_json::Value>,
    result: oneshot::Sender<PluginResult<Option<PluginOnLoadResult>>>,
  },
  OnEnd {
    plugin_ids: Vec<u32>,
    args: Vec<serde_json::Value>,
    result: oneshot::Sender<PluginResult<PluginOnEndResult>>,
  },
  OnDispose {
    plugin_ids: Vec<u32>,
    args: Vec<serde_json::Value>,
    result: oneshot::Sender<PluginResult<()>>,
  },
}

fn deserialize_bytes<'de, D>(
  deserializer: D,
) -> Result<Option<Vec<u8>>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  use serde::Deserialize;
  let s = Option::<serde_v8::StringOrBuffer>::deserialize(deserializer)?;
  Ok(s.map(|s| s.to_vec()))
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PluginOnLoadResult {
  pub plugin_name: Option<String>,
  pub errors: Option<Vec<Message>>,
  pub warnings: Option<Vec<Message>>,
  #[serde(deserialize_with = "deserialize_bytes")]
  pub contents: Option<Vec<u8>>,
  pub resolve_dir: Option<String>,
  pub loader: Option<String>,
  pub plugin_data: Option<u32>,
  pub watch_files: Option<Vec<String>>,
  pub watch_dirs: Option<Vec<String>>,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PluginOnEndResult {
  pub errors: Option<Vec<Message>>,
  pub warnings: Option<Vec<Message>>,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PluginOnStartResult {
  pub errors: Option<Vec<Message>>,
  pub warnings: Option<Vec<Message>>,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PluginOnResolveResult {
  pub path: Option<String>,
  pub external: Option<bool>,
  pub side_effects: Option<bool>,
  pub namespace: Option<String>,
  pub suffix: Option<String>,
  pub plugin_data: Option<u32>,
  pub errors: Option<Vec<Message>>,
  pub warnings: Option<Vec<Message>>,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PluginResult<T> {
  pub plugin_id: Option<u32>,
  pub result: T,
}

trait ResultExt<T> {
  fn unwrap_infallible(self) -> T;
}

impl<T> ResultExt<T> for Result<T, Infallible> {
  fn unwrap_infallible(self) -> T {
    match self {
      Ok(value) => value,
      Err(never) => match never {},
    }
  }
}

type Bt = std::backtrace::Backtrace;

#[derive(thiserror::Error, Debug, deno_error::JsError)]
pub enum PluginExecError {
  #[class(generic)]
  #[error("serde_v8 error: {0}; from {1}")]
  SerdeV8(serde_v8::Error, Bt),
}

impl From<serde_v8::Error> for PluginExecError {
  fn from(e: serde_v8::Error) -> Self {
    PluginExecError::SerdeV8(e, std::backtrace::Backtrace::capture())
  }
}

#[derive(Debug)]
pub enum PluginExecResultSender {
  OnStart(oneshot::Sender<PluginResult<PluginOnStartResult>>),
  OnResolve(oneshot::Sender<PluginResult<Option<PluginOnResolveResult>>>),
  OnLoad(oneshot::Sender<PluginResult<Option<PluginOnLoadResult>>>),
  OnEnd(oneshot::Sender<PluginResult<PluginOnEndResult>>),
  OnDispose(oneshot::Sender<PluginResult<()>>),
}

#[derive(Debug)]
pub struct PluginExecResultSenderWrapper {
  sender: std::cell::RefCell<Option<PluginExecResultSender>>,
}

impl From<PluginExecResultSender> for PluginExecResultSenderWrapper {
  fn from(sender: PluginExecResultSender) -> Self {
    Self {
      sender: std::cell::RefCell::new(Some(sender)),
    }
  }
}

impl deno_core::GarbageCollected for PluginExecResultSenderWrapper {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"PluginExecResultSenderWrapper"
  }
}

impl PluginRequest {
  fn to_args<'s>(
    self,
    scope: &mut v8::HandleScope<'s>,
  ) -> Result<Vec<v8::Local<'s, v8::Value>>, PluginExecError> {
    let mut v8_args = Vec::new();
    match self {
      PluginRequest::OnStart {
        plugin_ids,
        args,
        result,
      } => {
        v8_args.push(PluginHook::OnStart.to_v8(scope).unwrap_infallible());
        v8_args
          .push(PluginHookType::Sequential.to_v8(scope).unwrap_infallible());
        v8_args.push(serde_v8::to_v8(scope, plugin_ids)?);
        let sender = PluginExecResultSenderWrapper::from(
          PluginExecResultSender::OnStart(result),
        );
        v8_args.push(deno_core::cppgc::make_cppgc_object(scope, sender).into());
        v8_args.push(serde_v8::to_v8(scope, args)?);
      }
      PluginRequest::OnResolve {
        plugin_ids,
        args,
        result,
      } => {
        v8_args.push(PluginHook::OnResolve.to_v8(scope).unwrap_infallible());
        v8_args.push(PluginHookType::First.to_v8(scope).unwrap_infallible());
        v8_args.push(serde_v8::to_v8(scope, plugin_ids)?);
        let sender = PluginExecResultSenderWrapper::from(
          PluginExecResultSender::OnResolve(result),
        );
        v8_args.push(deno_core::cppgc::make_cppgc_object(scope, sender).into());
        v8_args.push(serde_v8::to_v8(scope, args)?);
      }
      PluginRequest::OnLoad {
        plugin_ids,
        args,
        result,
      } => {
        v8_args.push(PluginHook::OnLoad.to_v8(scope).unwrap_infallible());
        v8_args.push(PluginHookType::First.to_v8(scope).unwrap_infallible());
        v8_args.push(serde_v8::to_v8(scope, plugin_ids)?);
        let sender = PluginExecResultSenderWrapper::from(
          PluginExecResultSender::OnLoad(result),
        );
        v8_args.push(deno_core::cppgc::make_cppgc_object(scope, sender).into());
        v8_args.push(serde_v8::to_v8(scope, args)?);
      }
      PluginRequest::OnEnd {
        plugin_ids,
        args,
        result,
      } => {
        v8_args.push(PluginHook::OnEnd.to_v8(scope).unwrap_infallible());
        v8_args
          .push(PluginHookType::Sequential.to_v8(scope).unwrap_infallible());
        v8_args.push(serde_v8::to_v8(scope, plugin_ids)?);
        let sender = PluginExecResultSenderWrapper::from(
          PluginExecResultSender::OnEnd(result),
        );
        v8_args.push(deno_core::cppgc::make_cppgc_object(scope, sender).into());
        v8_args.push(serde_v8::to_v8(scope, args)?);
      }
      PluginRequest::OnDispose {
        plugin_ids,
        args,
        result,
      } => {
        v8_args.push(PluginHook::OnDispose.to_v8(scope).unwrap_infallible());
        v8_args
          .push(PluginHookType::Sequential.to_v8(scope).unwrap_infallible());
        v8_args.push(serde_v8::to_v8(scope, plugin_ids)?);
        let sender = PluginExecResultSenderWrapper::from(
          PluginExecResultSender::OnDispose(result),
        );
        v8_args.push(deno_core::cppgc::make_cppgc_object(scope, sender).into());
        v8_args.push(serde_v8::to_v8(scope, args)?);
      }
    }
    Ok(v8_args)
  }
}

#[derive(Clone, Copy, Debug)]
enum PluginHookType {
  First = 0,
  Sequential = 1,
}

impl PluginHookType {
  fn to_v8<'s>(
    self,
    scope: &mut v8::HandleScope<'s>,
  ) -> Result<v8::Local<'s, v8::Value>, Infallible> {
    Ok(v8::Integer::new(scope, self as i32).into())
  }
}

#[derive(Clone, Copy, Debug)]
enum PluginHook {
  OnStart = 0,
  OnResolve = 1,
  OnLoad = 2,
  OnEnd = 3,
  OnDispose = 4,
}

impl PluginHook {
  fn to_v8<'s>(
    self,
    scope: &mut v8::HandleScope<'s>,
  ) -> Result<v8::Local<'s, v8::Value>, Infallible> {
    Ok(v8::Integer::new(scope, self as i32).into())
  }
}

#[op2]
impl PluginExecResultSenderWrapper {
  #[fast]
  #[rename("sendResult")]
  fn send_result<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
    res: v8::Local<'s, v8::Value>,
  ) -> Result<(), PluginExecError> {
    let sender = self.sender.borrow_mut().take().unwrap();
    sender.send_result(scope, res)
  }
}

impl PluginExecResultSender {
  fn send_result<'s>(
    self,
    scope: &mut v8::HandleScope<'s>,
    res: v8::Local<'s, v8::Value>,
  ) -> Result<(), PluginExecError> {
    match self {
      PluginExecResultSender::OnStart(result) => {
        let res = serde_v8::from_v8(scope, res)?;
        let _ = result.send(res);
      }
      PluginExecResultSender::OnResolve(result) => {
        let res = serde_v8::from_v8(scope, res)?;
        let _ = result.send(res);
      }
      PluginExecResultSender::OnLoad(result) => {
        let res = serde_v8::from_v8(scope, res)?;
        let _ = result.send(res);
      }
      PluginExecResultSender::OnEnd(result) => {
        let res = serde_v8::from_v8(scope, res)?;
        let _ = result.send(res);
      }
      PluginExecResultSender::OnDispose(result) => {
        let res = serde_v8::from_v8(scope, res)?;
        let _ = result.send(res);
      }
    }
    Ok(())
  }
}

#[derive(Debug)]
pub struct Plugins {
  pub sender: tokio::sync::mpsc::Sender<PluginRequest>,
  pub info: Vec<PluginInfo>,
}
