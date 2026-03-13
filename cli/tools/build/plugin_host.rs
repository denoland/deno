// Copyright 2018-2026 the Deno authors. MIT license.

//! V8-based plugin host for the bundler.
//!
//! Spawns a dedicated thread with a `MainWorker` (V8 isolate) that loads
//! user-provided JS/TS build plugins. Communication happens via
//! `mpsc`/`oneshot` channels, following the same pattern as lint plugins
//! (`cli/tools/lint/plugins.rs`).
//!
//! The JS plugin API follows esbuild's model:
//!
//! ```js
//! export default function myPlugin(build) {
//!   build.onResolve({ filter: /\.css$/ }, (args) => { ... });
//!   build.onLoad({ filter: /\.css$/ }, (args) => { ... });
//!   build.onTransform({ filter: /\.vue$/ }, (args) => { ... });
//!   build.onWatchChange((args) => { ... });
//! }
//! ```

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_config::deno_json::PluginConfig;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::FutureExt;
use deno_core::v8;
use deno_core::PollEventLoopOptions;
use deno_path_util::resolve_url_or_path;
use deno_runtime::tokio_util;
use deno_runtime::worker::MainWorker;
use deno_runtime::WorkerExecutionMode;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::args::BuildFlags;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::PermissionFlags;
use crate::factory::CliFactory;

use deno_bundler::loader::Loader;
use deno_bundler::plugin::HookFilter;
use deno_bundler::plugin::LoadArgs;
use deno_bundler::plugin::LoadResult;
use deno_bundler::plugin::OnLoad;
use deno_bundler::plugin::OnResolve;
use deno_bundler::plugin::OnTransform;
use deno_bundler::plugin::OnWatchChange;
use deno_bundler::plugin::Plugin;
use deno_bundler::plugin::PluginBuild;
use deno_bundler::plugin::ResolveArgs;
use deno_bundler::plugin::ResolveResult;
use deno_bundler::plugin::TransformArgs;
use deno_bundler::plugin::TransformResult;
use deno_bundler::plugin::WatchChangeArgs;
use deno_bundler::plugin::WatchChangeResult;

// ---------------------------------------------------------------------------
// Request / Response types for channel communication
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum PluginHostRequest {
  LoadPlugins {
    specifiers: Vec<ModuleSpecifier>,
    tx: oneshot::Sender<PluginHostResponse>,
  },
  Resolve {
    specifier: String,
    importer: String,
    namespace: String,
    kind: String,
    tx: oneshot::Sender<PluginHostResponse>,
  },
  Load {
    path: String,
    namespace: String,
    tx: oneshot::Sender<PluginHostResponse>,
  },
  Transform {
    content: String,
    path: String,
    namespace: String,
    loader: String,
    source_map: Option<String>,
    tx: oneshot::Sender<PluginHostResponse>,
  },
  WatchChange {
    path: String,
    tx: oneshot::Sender<PluginHostResponse>,
  },
}

pub enum PluginHostResponse {
  LoadPlugins(Result<PluginInfo, AnyError>),
  Resolve(Result<Option<JsResolveResult>, AnyError>),
  Load(Result<Option<JsLoadResult>, AnyError>),
  Transform(Result<Option<JsTransformResult>, AnyError>),
  WatchChange(Result<JsWatchChangeResult, AnyError>),
}

impl std::fmt::Debug for PluginHostResponse {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::LoadPlugins(_) => f.debug_tuple("LoadPlugins").finish(),
      Self::Resolve(_) => f.debug_tuple("Resolve").finish(),
      Self::Load(_) => f.debug_tuple("Load").finish(),
      Self::Transform(_) => f.debug_tuple("Transform").finish(),
      Self::WatchChange(_) => f.debug_tuple("WatchChange").finish(),
    }
  }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
  pub plugin_count: usize,
  pub resolve_hook_count: usize,
  pub load_hook_count: usize,
  pub transform_hook_count: usize,
  pub watch_change_hook_count: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsResolveResult {
  pub path: String,
  #[serde(default = "default_namespace")]
  pub namespace: String,
  #[serde(default)]
  pub external: bool,
}

fn default_namespace() -> String {
  "file".to_string()
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct JsLoadResult {
  pub content: String,
  #[serde(default = "default_loader")]
  pub loader: String,
}

fn default_loader() -> String {
  "js".to_string()
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsTransformResult {
  pub content: Option<String>,
  pub loader: Option<String>,
  pub source_map: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsWatchChangeResult {
  #[serde(default)]
  pub add_entries: Vec<String>,
  #[serde(default)]
  pub remove_entries: Vec<String>,
}

// ---------------------------------------------------------------------------
// V8 static strings for function lookups
// ---------------------------------------------------------------------------

macro_rules! v8_static_strings {
  ($($ident:ident = $str:literal),* $(,)?) => {
    $(
      pub static $ident: deno_core::FastStaticString = deno_core::ascii_str!($str);
    )*
  };
}

v8_static_strings! {
  DEFAULT = "default",
  INSTALL_BUILD_PLUGINS = "installBuildPlugins",
  RUN_BUILD_ON_RESOLVE = "runBuildOnResolve",
  RUN_BUILD_ON_LOAD = "runBuildOnLoad",
  RUN_BUILD_ON_TRANSFORM = "runBuildOnTransform",
  RUN_BUILD_ON_WATCH_CHANGE = "runBuildOnWatchChange",
}

// ---------------------------------------------------------------------------
// BundlerPluginHostProxy — main-thread handle
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct BundlerPluginHostProxy {
  tx: mpsc::Sender<PluginHostRequest>,
  plugin_info: Option<PluginInfo>,
  #[allow(unused)]
  join_handle: std::thread::JoinHandle<Result<(), AnyError>>,
}

impl BundlerPluginHostProxy {
  pub async fn load_plugins(
    &mut self,
    specifiers: Vec<ModuleSpecifier>,
  ) -> Result<(), AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::LoadPlugins { specifiers, tx })
      .await?;

    if let Ok(val) = rx.await {
      let PluginHostResponse::LoadPlugins(result) = val else {
        unreachable!()
      };
      let info = result?;
      log::debug!(
        "Build plugins loaded: {} plugins, {} resolve hooks, {} load hooks, {} transform hooks",
        info.plugin_count,
        info.resolve_hook_count,
        info.load_hook_count,
        info.transform_hook_count,
      );
      self.plugin_info = Some(info);
      return Ok(());
    }
    deno_core::anyhow::bail!("Plugin host has closed")
  }

  pub async fn resolve(
    &self,
    specifier: &str,
    importer: &str,
    namespace: &str,
    kind: &str,
  ) -> Result<Option<JsResolveResult>, AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::Resolve {
        specifier: specifier.to_string(),
        importer: importer.to_string(),
        namespace: namespace.to_string(),
        kind: kind.to_string(),
        tx,
      })
      .await?;

    if let Ok(PluginHostResponse::Resolve(result)) = rx.await {
      return result;
    }
    deno_core::anyhow::bail!("Plugin host has closed")
  }

  pub async fn load(
    &self,
    path: &str,
    namespace: &str,
  ) -> Result<Option<JsLoadResult>, AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::Load {
        path: path.to_string(),
        namespace: namespace.to_string(),
        tx,
      })
      .await?;

    if let Ok(PluginHostResponse::Load(result)) = rx.await {
      return result;
    }
    deno_core::anyhow::bail!("Plugin host has closed")
  }

  pub async fn transform(
    &self,
    content: &str,
    path: &str,
    namespace: &str,
    loader: &str,
    source_map: Option<&str>,
  ) -> Result<Option<JsTransformResult>, AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::Transform {
        content: content.to_string(),
        path: path.to_string(),
        namespace: namespace.to_string(),
        loader: loader.to_string(),
        source_map: source_map.map(|s| s.to_string()),
        tx,
      })
      .await?;

    if let Ok(PluginHostResponse::Transform(result)) = rx.await {
      return result;
    }
    deno_core::anyhow::bail!("Plugin host has closed")
  }

  pub async fn watch_change(
    &self,
    path: &str,
  ) -> Result<JsWatchChangeResult, AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::WatchChange {
        path: path.to_string(),
        tx,
      })
      .await?;

    if let Ok(PluginHostResponse::WatchChange(result)) = rx.await {
      return result;
    }
    deno_core::anyhow::bail!("Plugin host has closed")
  }

  /// Whether any JS plugins are loaded.
  #[allow(unused)]
  pub fn has_plugins(&self) -> bool {
    self
      .plugin_info
      .as_ref()
      .is_some_and(|info| info.plugin_count > 0)
  }

  /// Whether any resolve hooks are registered.
  pub fn has_resolve_hooks(&self) -> bool {
    self
      .plugin_info
      .as_ref()
      .is_some_and(|info| info.resolve_hook_count > 0)
  }

  /// Whether any load hooks are registered.
  pub fn has_load_hooks(&self) -> bool {
    self
      .plugin_info
      .as_ref()
      .is_some_and(|info| info.load_hook_count > 0)
  }

  /// Whether any transform hooks are registered.
  pub fn has_transform_hooks(&self) -> bool {
    self
      .plugin_info
      .as_ref()
      .is_some_and(|info| info.transform_hook_count > 0)
  }

  /// Whether any watch change hooks are registered.
  pub fn has_watch_change_hooks(&self) -> bool {
    self
      .plugin_info
      .as_ref()
      .is_some_and(|info| info.watch_change_hook_count > 0)
  }
}

// ---------------------------------------------------------------------------
// BundlerPluginHost — runs on a dedicated thread
// ---------------------------------------------------------------------------

struct BundlerPluginHost {
  worker: MainWorker,
  install_plugins_fn: Rc<v8::Global<v8::Function>>,
  on_resolve_fn: Rc<v8::Global<v8::Function>>,
  on_load_fn: Rc<v8::Global<v8::Function>>,
  on_transform_fn: Rc<v8::Global<v8::Function>>,
  on_watch_change_fn: Rc<v8::Global<v8::Function>>,
  rx: mpsc::Receiver<PluginHostRequest>,
}

async fn create_plugin_host_inner(
  rx_req: mpsc::Receiver<PluginHostRequest>,
) -> Result<BundlerPluginHost, AnyError> {
  let flags = Flags {
    subcommand: DenoSubcommand::Build(BuildFlags { watch: false, env_file: None }),
    permissions: PermissionFlags {
      allow_env: Some(vec![]),
      allow_read: Some(vec![]),
      no_prompt: true,
      ..Default::default()
    },
    ..Default::default()
  };
  let flags = Arc::new(flags);
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let main_module =
    resolve_url_or_path("./$deno$build.mts", cli_options.initial_cwd()).unwrap();
  let permissions = factory.root_permissions_container()?.clone();
  let worker_factory = factory.create_cli_main_worker_factory().await?;

  let worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Run,
      main_module.clone(),
      vec![],
      vec![],
      permissions,
      vec![crate::ops::build::deno_build_ext::init()],
      Default::default(),
      None,
    )
    .await?;

  let mut worker = worker.into_main_worker();
  let runtime = &mut worker.js_runtime;

  let obj = runtime.execute_script("build.js", "Deno[Deno.internal]")?;

  let (
    install_plugins_fn,
    on_resolve_fn,
    on_load_fn,
    on_transform_fn,
    on_watch_change_fn,
  ) = {
    deno_core::scope!(scope, runtime);
    let module_exports: v8::Local<v8::Object> =
      v8::Local::new(scope, obj).try_into().unwrap();

    macro_rules! get_fn {
      ($name:expr) => {{
        let key = $name.v8_string(scope).unwrap();
        let val = module_exports.get(scope, key.into()).unwrap();
        let func: v8::Local<v8::Function> = val.try_into().unwrap();
        Rc::new(v8::Global::new(scope, func))
      }};
    }

    (
      get_fn!(INSTALL_BUILD_PLUGINS),
      get_fn!(RUN_BUILD_ON_RESOLVE),
      get_fn!(RUN_BUILD_ON_LOAD),
      get_fn!(RUN_BUILD_ON_TRANSFORM),
      get_fn!(RUN_BUILD_ON_WATCH_CHANGE),
    )
  };

  Ok(BundlerPluginHost {
    worker,
    install_plugins_fn,
    on_resolve_fn,
    on_load_fn,
    on_transform_fn,
    on_watch_change_fn,
    rx: rx_req,
  })
}

impl BundlerPluginHost {
  fn create() -> Result<BundlerPluginHostProxy, AnyError> {
    let (tx_req, rx_req) = mpsc::channel(10);

    let join_handle = std::thread::spawn(move || {
      log::debug!("Build PluginHost thread spawned");
      let start = std::time::Instant::now();
      let fut = async move {
        let host = create_plugin_host_inner(rx_req).await?;
        host.run_loop().await?;
        log::debug!(
          "Build PluginHost thread finished, took {:?}",
          std::time::Instant::now() - start
        );
        Ok(())
      }
      .boxed_local();
      tokio_util::create_and_run_current_thread(fut)
    });

    Ok(BundlerPluginHostProxy {
      tx: tx_req,
      plugin_info: None,
      join_handle,
    })
  }

  async fn run_loop(mut self) -> Result<(), AnyError> {
    log::debug!("Build PluginHost is waiting for message");
    while let Some(req) = self.rx.recv().await {
      match req {
        PluginHostRequest::LoadPlugins { specifiers, tx } => {
          let r = self.load_plugins(specifiers).await;
          let _ = tx.send(PluginHostResponse::LoadPlugins(r));
        }
        PluginHostRequest::Resolve {
          specifier,
          importer,
          namespace,
          kind,
          tx,
        } => {
          let r = self.run_resolve(&specifier, &importer, &namespace, &kind);
          let _ = tx.send(PluginHostResponse::Resolve(r));
        }
        PluginHostRequest::Load {
          path,
          namespace,
          tx,
        } => {
          let r = self.run_load(&path, &namespace);
          let _ = tx.send(PluginHostResponse::Load(r));
        }
        PluginHostRequest::Transform {
          content,
          path,
          namespace,
          loader,
          source_map,
          tx,
        } => {
          let r = self.run_transform(
            &content,
            &path,
            &namespace,
            &loader,
            source_map.as_deref(),
          );
          let _ = tx.send(PluginHostResponse::Transform(r));
        }
        PluginHostRequest::WatchChange { path, tx } => {
          let r = self.run_watch_change(&path);
          let _ = tx.send(PluginHostResponse::WatchChange(r));
        }
      }
    }
    log::debug!("Build PluginHost run loop finished");
    Ok(())
  }

  async fn load_plugins(
    &mut self,
    plugin_specifiers: Vec<ModuleSpecifier>,
  ) -> Result<PluginInfo, AnyError> {
    let mut load_futures = Vec::with_capacity(plugin_specifiers.len());
    for specifier in plugin_specifiers {
      let mod_id = self
        .worker
        .js_runtime
        .load_side_es_module(&specifier)
        .await?;
      let mod_future =
        self.worker.js_runtime.mod_evaluate(mod_id).boxed_local();
      load_futures.push((mod_future, mod_id));
    }

    self
      .worker
      .js_runtime
      .run_event_loop(PollEventLoopOptions::default())
      .await?;

    let mut plugin_handles = Vec::with_capacity(load_futures.len());
    for (fut, mod_id) in load_futures {
      fut.await?;
      let module =
        self.worker.js_runtime.get_module_namespace(mod_id).unwrap();
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let module_local = v8::Local::new(scope, module);
      let default_export_str = DEFAULT.v8_string(scope).unwrap();
      let default_export =
        module_local.get(scope, default_export_str.into()).unwrap();
      let default_export_global = v8::Global::new(scope, default_export);
      plugin_handles.push(default_export_global);
    }

    // Call installBuildPlugins(pluginModules)
    deno_core::scope!(scope, &mut self.worker.js_runtime);
    let install_fn =
      v8::Local::new(scope, &*self.install_plugins_fn.clone());
    let undefined = v8::undefined(scope);

    let arr =
      v8::Array::new(scope, plugin_handles.len().try_into().unwrap());
    for (idx, handle) in plugin_handles.into_iter().enumerate() {
      let local = v8::Local::new(scope, handle);
      // Wrap in an object with `default` property for the JS side
      let obj = v8::Object::new(scope);
      let default_key = DEFAULT.v8_string(scope).unwrap();
      obj.set(scope, default_key.into(), local).unwrap();
      arr
        .set_index(scope, idx.try_into().unwrap(), obj.into())
        .unwrap();
    }

    let result = {
      v8::tc_scope!(tc_scope, scope);
      let result =
        install_fn.call(tc_scope, undefined.into(), &[arr.into()]);
      if let Some(exception) = tc_scope.exception() {
        let error = JsError::from_v8_exception(tc_scope, exception);
        return Err(error.into());
      }
      result
    };

    let info: PluginInfo =
      deno_core::serde_v8::from_v8(scope, result.unwrap())?;
    Ok(info)
  }

  fn run_resolve(
    &mut self,
    specifier: &str,
    importer: &str,
    namespace: &str,
    kind: &str,
  ) -> Result<Option<JsResolveResult>, AnyError> {
    deno_core::scope!(scope, &mut self.worker.js_runtime);
    let func = v8::Local::new(scope, &*self.on_resolve_fn);
    let undefined = v8::undefined(scope);

    let specifier_v8 = v8::String::new(scope, specifier).unwrap();
    let importer_v8 = v8::String::new(scope, importer).unwrap();
    let namespace_v8 = v8::String::new(scope, namespace).unwrap();
    let kind_v8 = v8::String::new(scope, kind).unwrap();

    let result = {
      v8::tc_scope!(tc_scope, scope);
      let result = func.call(
        tc_scope,
        undefined.into(),
        &[
          specifier_v8.into(),
          importer_v8.into(),
          namespace_v8.into(),
          kind_v8.into(),
        ],
      );
      if let Some(exception) = tc_scope.exception() {
        let error = JsError::from_v8_exception(tc_scope, exception);
        return Err(error.into());
      }
      result
    };

    let result = result.unwrap();
    if result.is_null_or_undefined() {
      return Ok(None);
    }

    let resolve_result: JsResolveResult =
      deno_core::serde_v8::from_v8(scope, result)?;
    Ok(Some(resolve_result))
  }

  fn run_load(
    &mut self,
    path: &str,
    namespace: &str,
  ) -> Result<Option<JsLoadResult>, AnyError> {
    deno_core::scope!(scope, &mut self.worker.js_runtime);
    let func = v8::Local::new(scope, &*self.on_load_fn);
    let undefined = v8::undefined(scope);

    let path_v8 = v8::String::new(scope, path).unwrap();
    let namespace_v8 = v8::String::new(scope, namespace).unwrap();

    let result = {
      v8::tc_scope!(tc_scope, scope);
      let result = func.call(
        tc_scope,
        undefined.into(),
        &[path_v8.into(), namespace_v8.into()],
      );
      if let Some(exception) = tc_scope.exception() {
        let error = JsError::from_v8_exception(tc_scope, exception);
        return Err(error.into());
      }
      result
    };

    let result = result.unwrap();
    if result.is_null_or_undefined() {
      return Ok(None);
    }

    let load_result: JsLoadResult =
      deno_core::serde_v8::from_v8(scope, result)?;
    Ok(Some(load_result))
  }

  fn run_transform(
    &mut self,
    content: &str,
    path: &str,
    namespace: &str,
    loader: &str,
    source_map: Option<&str>,
  ) -> Result<Option<JsTransformResult>, AnyError> {
    deno_core::scope!(scope, &mut self.worker.js_runtime);
    let func = v8::Local::new(scope, &*self.on_transform_fn);
    let undefined = v8::undefined(scope);

    let content_v8 = v8::String::new(scope, content).unwrap();
    let path_v8 = v8::String::new(scope, path).unwrap();
    let namespace_v8 = v8::String::new(scope, namespace).unwrap();
    let loader_v8 = v8::String::new(scope, loader).unwrap();
    let source_map_v8: v8::Local<v8::Value> = match source_map {
      Some(sm) => v8::String::new(scope, sm).unwrap().into(),
      None => v8::null(scope).into(),
    };

    let result = {
      v8::tc_scope!(tc_scope, scope);
      let result = func.call(
        tc_scope,
        undefined.into(),
        &[
          content_v8.into(),
          path_v8.into(),
          namespace_v8.into(),
          loader_v8.into(),
          source_map_v8,
        ],
      );
      if let Some(exception) = tc_scope.exception() {
        let error = JsError::from_v8_exception(tc_scope, exception);
        return Err(error.into());
      }
      result
    };

    let result = result.unwrap();
    if result.is_null_or_undefined() {
      return Ok(None);
    }

    let transform_result: JsTransformResult =
      deno_core::serde_v8::from_v8(scope, result)?;
    Ok(Some(transform_result))
  }

  fn run_watch_change(
    &mut self,
    path: &str,
  ) -> Result<JsWatchChangeResult, AnyError> {
    deno_core::scope!(scope, &mut self.worker.js_runtime);
    let func = v8::Local::new(scope, &*self.on_watch_change_fn);
    let undefined = v8::undefined(scope);

    let path_v8 = v8::String::new(scope, path).unwrap();

    let result = {
      v8::tc_scope!(tc_scope, scope);
      let result =
        func.call(tc_scope, undefined.into(), &[path_v8.into()]);
      if let Some(exception) = tc_scope.exception() {
        let error = JsError::from_v8_exception(tc_scope, exception);
        return Err(error.into());
      }
      result
    };

    let result = result.unwrap();
    let watch_result: JsWatchChangeResult =
      deno_core::serde_v8::from_v8(scope, result)?;
    Ok(watch_result)
  }
}

// ---------------------------------------------------------------------------
// Public API: create and load
// ---------------------------------------------------------------------------

/// Resolve plugin config entries to module specifiers.
///
/// Each `PluginConfig` has a `specifier` string (e.g., `"./my-plugin.ts"`,
/// `"jsr:@example/plugin"`) and a `base` URL (the deno.json location).
pub fn resolve_plugin_specifiers(
  plugins: &[PluginConfig],
) -> Result<Vec<ModuleSpecifier>, AnyError> {
  let mut specifiers = Vec::with_capacity(plugins.len());
  for plugin in plugins {
    let specifier = deno_core::resolve_import(
      &plugin.specifier,
      plugin.base.as_str(),
    )
    .map_err(|e| {
      deno_core::anyhow::anyhow!(
        "Failed to resolve plugin \"{}\": {}",
        plugin.specifier,
        e
      )
    })?;
    specifiers.push(specifier);
  }
  Ok(specifiers)
}

/// Create a bundler plugin host and load plugin specifiers.
pub async fn create_and_load_plugins(
  plugin_specifiers: Vec<ModuleSpecifier>,
) -> Result<BundlerPluginHostProxy, AnyError> {
  let mut host_proxy = BundlerPluginHost::create()?;
  host_proxy.load_plugins(plugin_specifiers).await?;
  Ok(host_proxy)
}

// ---------------------------------------------------------------------------
// Bridge: BundlerPluginHostProxy → PluginDriver hooks
// ---------------------------------------------------------------------------

/// Converts a JS loader string (e.g., "ts", "tsx", "css") to a `Loader`.
fn loader_from_str(s: &str) -> Loader {
  match s {
    "js" => Loader::Js,
    "jsx" => Loader::Jsx,
    "ts" => Loader::Ts,
    "tsx" => Loader::Tsx,
    "css" => Loader::Css,
    "json" => Loader::Json,
    "html" => Loader::Html,
    "text" => Loader::Text,
    "binary" => Loader::Binary,
    "asset" => Loader::Asset,
    _ => Loader::Js,
  }
}

/// Converts a `Loader` to the JS loader string.
fn loader_to_str(loader: Loader) -> &'static str {
  match loader {
    Loader::Js => "js",
    Loader::Jsx => "jsx",
    Loader::Ts => "ts",
    Loader::Tsx => "tsx",
    Loader::Css => "css",
    Loader::Json => "json",
    Loader::Html => "html",
    Loader::Text => "text",
    Loader::Binary => "binary",
    Loader::Asset => "asset",
  }
}

/// A Plugin implementation that bridges to JS plugins via the host proxy.
///
/// This wraps the `BundlerPluginHostProxy` (which communicates with the V8
/// isolate on another thread) and implements the Rust `Plugin` trait so
/// JS plugin hooks can participate in the `PluginDriver`.
///
/// Note: The JS hooks are called synchronously from the plugin driver, but
/// the actual communication crosses thread boundaries via channels. This
/// requires a `tokio::runtime::Handle` to block on the async channel send/recv.
#[allow(unused)]
pub struct JsPluginBridge {
  proxy: Arc<BundlerPluginHostProxy>,
  rt_handle: tokio::runtime::Handle,
}

impl JsPluginBridge {
  pub fn new(
    proxy: Arc<BundlerPluginHostProxy>,
    rt_handle: tokio::runtime::Handle,
  ) -> Self {
    Self { proxy, rt_handle }
  }
}

impl Plugin for JsPluginBridge {
  fn name(&self) -> &str {
    "js-plugins"
  }

  fn setup(&self, build: &mut PluginBuild) {
    // Register bridge hooks that forward to the JS host.
    if self.proxy.has_resolve_hooks() {
      let proxy = self.proxy.clone();
      let handle = self.rt_handle.clone();
      build.on_resolve(
        HookFilter::new(regex::Regex::new(".*").unwrap()),
        Box::new(JsResolveBridge {
          proxy,
          handle,
        }),
      );
    }

    if self.proxy.has_load_hooks() {
      let proxy = self.proxy.clone();
      let handle = self.rt_handle.clone();
      build.on_load(
        HookFilter::new(regex::Regex::new(".*").unwrap()),
        Box::new(JsLoadBridge {
          proxy,
          handle,
        }),
      );
    }

    if self.proxy.has_transform_hooks() {
      let proxy = self.proxy.clone();
      let handle = self.rt_handle.clone();
      build.on_transform(
        HookFilter::new(regex::Regex::new(".*").unwrap()),
        Box::new(JsTransformBridge {
          proxy,
          handle,
        }),
      );
    }

    if self.proxy.has_watch_change_hooks() {
      let proxy = self.proxy.clone();
      let handle = self.rt_handle.clone();
      build.on_watch_change(
        HookFilter::new(regex::Regex::new(".*").unwrap()),
        Box::new(JsWatchChangeBridge {
          proxy,
          handle,
        }),
      );
    }
  }
}

// Individual bridge hook implementations

#[allow(unused)]
struct JsResolveBridge {
  proxy: Arc<BundlerPluginHostProxy>,
  handle: tokio::runtime::Handle,
}

impl OnResolve for JsResolveBridge {
  fn on_resolve(&self, args: &ResolveArgs) -> Option<ResolveResult> {
    let kind_str = match args.kind {
      deno_bundler::plugin::ResolveKind::Import => "import",
      deno_bundler::plugin::ResolveKind::DynamicImport => "dynamic-import",
      deno_bundler::plugin::ResolveKind::Require => "require",
      deno_bundler::plugin::ResolveKind::CssImport => "css-import",
      deno_bundler::plugin::ResolveKind::CssUrl => "css-url",
      deno_bundler::plugin::ResolveKind::HtmlAsset => "html-asset",
      deno_bundler::plugin::ResolveKind::Entry => "entry",
    };

    let result = self.handle.block_on(self.proxy.resolve(
      args.specifier,
      &args.importer.to_string_lossy(),
      args.namespace,
      kind_str,
    ));

    match result {
      Ok(Some(r)) => Some(ResolveResult {
        path: PathBuf::from(&r.path),
        namespace: r.namespace,
        external: r.external,
      }),
      Ok(None) => None,
      Err(e) => {
        log::error!("JS resolve plugin error: {}", e);
        None
      }
    }
  }
}

#[allow(unused)]
struct JsLoadBridge {
  proxy: Arc<BundlerPluginHostProxy>,
  handle: tokio::runtime::Handle,
}

impl OnLoad for JsLoadBridge {
  fn on_load(&self, args: &LoadArgs) -> Option<LoadResult> {
    let result = self.handle.block_on(
      self
        .proxy
        .load(&args.path.to_string_lossy(), args.namespace),
    );

    match result {
      Ok(Some(r)) => Some(LoadResult {
        content: r.content,
        loader: loader_from_str(&r.loader),
        asset_bytes: None,
      }),
      Ok(None) => None,
      Err(e) => {
        log::error!("JS load plugin error: {}", e);
        None
      }
    }
  }
}

#[allow(unused)]
struct JsTransformBridge {
  proxy: Arc<BundlerPluginHostProxy>,
  handle: tokio::runtime::Handle,
}

impl OnTransform for JsTransformBridge {
  fn on_transform(&self, args: &TransformArgs) -> Option<TransformResult> {
    let result = self.handle.block_on(self.proxy.transform(
      args.content,
      &args.path.to_string_lossy(),
      args.namespace,
      loader_to_str(args.loader),
      args.source_map,
    ));

    match result {
      Ok(Some(r)) => Some(TransformResult {
        content: r.content,
        loader: r.loader.map(|l| loader_from_str(&l)),
        source_map: r.source_map,
        program: None,
      }),
      Ok(None) => None,
      Err(e) => {
        log::error!("JS transform plugin error: {}", e);
        None
      }
    }
  }
}

#[allow(unused)]
struct JsWatchChangeBridge {
  proxy: Arc<BundlerPluginHostProxy>,
  handle: tokio::runtime::Handle,
}

impl OnWatchChange for JsWatchChangeBridge {
  fn on_watch_change(
    &self,
    args: &WatchChangeArgs,
  ) -> Option<WatchChangeResult> {
    let result = self
      .handle
      .block_on(self.proxy.watch_change(&args.path.to_string_lossy()));

    match result {
      Ok(r) => {
        if r.add_entries.is_empty() && r.remove_entries.is_empty() {
          None
        } else {
          Some(WatchChangeResult {
            add_entries: r.add_entries,
            remove_entries: r.remove_entries,
          })
        }
      }
      Err(e) => {
        log::error!("JS watch change plugin error: {}", e);
        None
      }
    }
  }
}
