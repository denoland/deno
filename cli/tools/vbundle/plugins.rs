// Copyright 2018-2026 the Deno authors. MIT license.

//! Plugin host for the bundler.
//!
//! This module implements a plugin system similar to Vite/Rollup, allowing
//! JavaScript plugins to transform files. The plugin host runs in a separate
//! OS thread with its own V8 isolate, communicating with the main bundler
//! via channels.
//!
//! Unlike the linter plugin system, this sends source code strings to plugins
//! rather than serialized AST, following the Vite/Rollup plugin model.

use std::rc::Rc;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::PollEventLoopOptions;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_core::v8;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::tokio_util;
use deno_runtime::worker::MainWorker;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use super::types::ChunkInfo;
use super::types::LoadResult;
use super::types::PluginInfo;
use super::types::RenderChunkResult;
use super::types::ResolveOptions;
use super::types::ResolveResult;
use super::types::TransformResult;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::PermissionFlags;
use crate::factory::CliFactory;

/// Request types sent from the main thread to the plugin host.
#[derive(Debug)]
pub enum PluginHostRequest {
  /// Load and initialize plugins from the given specifiers.
  LoadPlugins {
    specifiers: Vec<ModuleSpecifier>,
    tx: oneshot::Sender<PluginHostResponse>,
  },
  /// Call buildStart hooks.
  BuildStart {
    tx: oneshot::Sender<PluginHostResponse>,
  },
  /// Call buildEnd hooks.
  BuildEnd {
    tx: oneshot::Sender<PluginHostResponse>,
  },
  /// Resolve a module specifier.
  Resolve {
    source: String,
    importer: Option<String>,
    options: ResolveOptions,
    tx: oneshot::Sender<PluginHostResponse>,
  },
  /// Load a module's source code.
  Load {
    id: String,
    tx: oneshot::Sender<PluginHostResponse>,
  },
  /// Transform source code.
  Transform {
    id: String,
    code: String,
    tx: oneshot::Sender<PluginHostResponse>,
  },
  /// Transform chunk code before emission.
  RenderChunk {
    code: String,
    chunk: ChunkInfo,
    tx: oneshot::Sender<PluginHostResponse>,
  },
  /// Called after bundle generation.
  GenerateBundle {
    bundle: std::collections::HashMap<String, ChunkInfo>,
    tx: oneshot::Sender<PluginHostResponse>,
  },
  /// Shutdown the plugin host.
  Shutdown,
}

/// Response types sent from the plugin host to the main thread.
pub enum PluginHostResponse {
  /// Result of loading plugins.
  LoadPlugins(Result<Vec<PluginInfo>, AnyError>),
  /// Result of buildStart hook.
  BuildStart(Result<(), AnyError>),
  /// Result of buildEnd hook.
  BuildEnd(Result<(), AnyError>),
  /// Result of resolving a specifier.
  Resolve(Result<Option<ResolveResult>, AnyError>),
  /// Result of loading a module.
  Load(Result<Option<LoadResult>, AnyError>),
  /// Result of transforming code.
  Transform(Result<Option<TransformResult>, AnyError>),
  /// Result of renderChunk hook.
  RenderChunk(Result<Option<RenderChunkResult>, AnyError>),
  /// Result of generateBundle hook.
  GenerateBundle(Result<(), AnyError>),
}

impl std::fmt::Debug for PluginHostResponse {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::LoadPlugins(_) => f.debug_tuple("LoadPlugins").finish(),
      Self::BuildStart(_) => f.debug_tuple("BuildStart").finish(),
      Self::BuildEnd(_) => f.debug_tuple("BuildEnd").finish(),
      Self::Resolve(_) => f.debug_tuple("Resolve").finish(),
      Self::Load(_) => f.debug_tuple("Load").finish(),
      Self::Transform(_) => f.debug_tuple("Transform").finish(),
      Self::RenderChunk(_) => f.debug_tuple("RenderChunk").finish(),
      Self::GenerateBundle(_) => f.debug_tuple("GenerateBundle").finish(),
    }
  }
}

/// Logger for plugin output.
#[derive(Clone, Debug)]
pub struct PluginLogger {
  print: fn(&str, bool),
}

impl PluginLogger {
  pub fn new(print: fn(&str, bool)) -> Self {
    Self { print }
  }

  pub fn log(&self, msg: &str) {
    (self.print)(msg, false);
  }

  pub fn error(&self, msg: &str) {
    (self.print)(msg, true);
  }
}

impl Default for PluginLogger {
  fn default() -> Self {
    Self::new(|msg, is_error| {
      if is_error {
        eprintln!("{}", msg);
      } else {
        println!("{}", msg);
      }
    })
  }
}

macro_rules! v8_static_strings {
  ($($ident:ident = $str:literal),* $(,)?) => {
    $(
      pub static $ident: deno_core::FastStaticString = deno_core::ascii_str!($str);
    )*
  };
}

v8_static_strings! {
  DEFAULT = "default",
  INSTALL_PLUGINS = "installPlugins",
  BUILD_START = "buildStart",
  BUILD_END = "buildEnd",
  RESOLVE_ID = "resolveId",
  LOAD = "load",
  TRANSFORM = "transform",
  RENDER_CHUNK = "renderChunk",
  GENERATE_BUNDLE = "generateBundle",
}

/// Proxy for communicating with the plugin host from the main thread.
///
/// This is the main interface used by the bundler to interact with plugins.
#[derive(Debug)]
pub struct PluginHostProxy {
  tx: mpsc::Sender<PluginHostRequest>,
  pub(crate) plugin_info: Arc<Mutex<Vec<PluginInfo>>>,
  #[allow(unused)]
  join_handle: std::thread::JoinHandle<Result<(), AnyError>>,
}

impl PluginHostProxy {
  /// Get information about loaded plugins.
  pub fn get_plugins(&self) -> Vec<PluginInfo> {
    self.plugin_info.lock().clone()
  }

  /// Check if any plugin handles a given file extension.
  pub fn handles_extension(&self, ext: &str) -> bool {
    let infos = self.plugin_info.lock();
    infos.iter().any(|info| {
      info.extensions.iter().any(|e| {
        e.trim_start_matches('.') == ext.trim_start_matches('.')
      })
    })
  }

  /// Load plugins from the given specifiers.
  pub async fn load_plugins(
    &self,
    specifiers: Vec<ModuleSpecifier>,
  ) -> Result<(), AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::LoadPlugins { specifiers, tx })
      .await?;

    if let Ok(PluginHostResponse::LoadPlugins(result)) = rx.await {
      let infos = result?;
      *self.plugin_info.lock() = infos;
      return Ok(());
    }
    bail!("Plugin host has closed")
  }

  /// Resolve a module specifier through plugins.
  ///
  /// Returns `None` if no plugin handles this specifier.
  pub async fn resolve(
    &self,
    source: &str,
    importer: Option<&str>,
    options: ResolveOptions,
  ) -> Result<Option<ResolveResult>, AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::Resolve {
        source: source.to_string(),
        importer: importer.map(|s| s.to_string()),
        options,
        tx,
      })
      .await?;

    if let Ok(PluginHostResponse::Resolve(result)) = rx.await {
      return result;
    }
    bail!("Plugin host has closed")
  }

  /// Load a module's source code through plugins.
  ///
  /// Returns `None` if no plugin handles this module.
  pub async fn load(&self, id: &str) -> Result<Option<LoadResult>, AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::Load {
        id: id.to_string(),
        tx,
      })
      .await?;

    if let Ok(PluginHostResponse::Load(result)) = rx.await {
      return result;
    }
    bail!("Plugin host has closed")
  }

  /// Transform source code through plugins.
  ///
  /// Returns `None` if no plugin transforms this module.
  pub async fn transform(
    &self,
    id: &str,
    code: &str,
  ) -> Result<Option<TransformResult>, AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::Transform {
        id: id.to_string(),
        code: code.to_string(),
        tx,
      })
      .await?;

    if let Ok(PluginHostResponse::Transform(result)) = rx.await {
      return result;
    }
    bail!("Plugin host has closed")
  }

  /// Call buildStart hooks on all plugins.
  pub async fn build_start(&self) -> Result<(), AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::BuildStart { tx })
      .await?;

    if let Ok(PluginHostResponse::BuildStart(result)) = rx.await {
      return result;
    }
    bail!("Plugin host has closed")
  }

  /// Call buildEnd hooks on all plugins.
  pub async fn build_end(&self) -> Result<(), AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::BuildEnd { tx })
      .await?;

    if let Ok(PluginHostResponse::BuildEnd(result)) = rx.await {
      return result;
    }
    bail!("Plugin host has closed")
  }

  /// Transform chunk code through plugins.
  ///
  /// Returns `None` if no plugin transforms this chunk.
  pub async fn render_chunk(
    &self,
    code: &str,
    chunk: ChunkInfo,
  ) -> Result<Option<RenderChunkResult>, AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::RenderChunk {
        code: code.to_string(),
        chunk,
        tx,
      })
      .await?;

    if let Ok(PluginHostResponse::RenderChunk(result)) = rx.await {
      return result;
    }
    bail!("Plugin host has closed")
  }

  /// Call generateBundle hooks on all plugins.
  pub async fn generate_bundle(
    &self,
    bundle: std::collections::HashMap<String, ChunkInfo>,
  ) -> Result<(), AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::GenerateBundle { bundle, tx })
      .await?;

    if let Ok(PluginHostResponse::GenerateBundle(result)) = rx.await {
      return result;
    }
    bail!("Plugin host has closed")
  }

  /// Shutdown the plugin host.
  pub async fn shutdown(&self) -> Result<(), AnyError> {
    let _ = self.tx.send(PluginHostRequest::Shutdown).await;
    Ok(())
  }
}

/// The plugin host that runs in a separate thread.
pub struct PluginHost {
  worker: MainWorker,
  install_plugins_fn: Rc<v8::Global<v8::Function>>,
  build_start_fn: Rc<v8::Global<v8::Function>>,
  build_end_fn: Rc<v8::Global<v8::Function>>,
  resolve_id_fn: Rc<v8::Global<v8::Function>>,
  load_fn: Rc<v8::Global<v8::Function>>,
  transform_fn: Rc<v8::Global<v8::Function>>,
  render_chunk_fn: Rc<v8::Global<v8::Function>>,
  generate_bundle_fn: Rc<v8::Global<v8::Function>>,
  rx: mpsc::Receiver<PluginHostRequest>,
}

impl PluginHost {
  /// Create a new plugin host, spawning a separate thread.
  pub fn create(logger: PluginLogger) -> Result<PluginHostProxy, AnyError> {
    let (tx_req, rx_req) = mpsc::channel(10);

    let logger_ = logger.clone();
    let join_handle = std::thread::spawn(move || {
      let logger = logger_;
      log::debug!("Vbundle PluginHost thread spawned");
      let start = std::time::Instant::now();
      let fut = async move {
        let runner = match create_plugin_runner_inner(logger.clone(), rx_req).await {
          Ok(runner) => runner,
          Err(e) => {
            log::error!("Vbundle PluginHost initialization failed: {}", e);
            return Err(e);
          }
        };
        log::debug!("Vbundle PluginHost running loop");
        runner.run_loop().await?;
        log::debug!(
          "Vbundle PluginHost thread finished, took {:?}",
          std::time::Instant::now() - start
        );
        Ok(())
      }
      .boxed_local();
      tokio_util::create_and_run_current_thread(fut)
    });

    let proxy = PluginHostProxy {
      tx: tx_req,
      plugin_info: Arc::new(Mutex::new(vec![])),
      join_handle,
    };

    Ok(proxy)
  }

  /// Run the plugin host event loop.
  async fn run_loop(mut self) -> Result<(), AnyError> {
    log::debug!("Vbundle PluginHost is waiting for message");
    while let Some(req) = self.rx.recv().await {
      log::debug!("Vbundle PluginHost has received a message");
      match req {
        PluginHostRequest::LoadPlugins { specifiers, tx } => {
          let r = self.load_plugins(specifiers).await;
          let _ = tx.send(PluginHostResponse::LoadPlugins(r));
        }
        PluginHostRequest::BuildStart { tx } => {
          let r = self.build_start().await;
          let _ = tx.send(PluginHostResponse::BuildStart(r));
        }
        PluginHostRequest::BuildEnd { tx } => {
          let r = self.build_end().await;
          let _ = tx.send(PluginHostResponse::BuildEnd(r));
        }
        PluginHostRequest::Resolve {
          source,
          importer,
          options,
          tx,
        } => {
          let r = self.resolve_id(&source, importer.as_deref(), &options).await;
          let _ = tx.send(PluginHostResponse::Resolve(r));
        }
        PluginHostRequest::Load { id, tx } => {
          let r = self.load(&id).await;
          let _ = tx.send(PluginHostResponse::Load(r));
        }
        PluginHostRequest::Transform { id, code, tx } => {
          let r = self.transform(&id, &code).await;
          let _ = tx.send(PluginHostResponse::Transform(r));
        }
        PluginHostRequest::RenderChunk { code, chunk, tx } => {
          let r = self.render_chunk(&code, &chunk).await;
          let _ = tx.send(PluginHostResponse::RenderChunk(r));
        }
        PluginHostRequest::GenerateBundle { bundle, tx } => {
          let r = self.generate_bundle(&bundle).await;
          let _ = tx.send(PluginHostResponse::GenerateBundle(r));
        }
        PluginHostRequest::Shutdown => {
          log::debug!("Vbundle PluginHost shutting down");
          break;
        }
      }
    }
    log::debug!("Vbundle PluginHost run loop finished");
    Ok(())
  }

  /// Load plugins from the given module specifiers.
  async fn load_plugins(
    &mut self,
    plugin_specifiers: Vec<ModuleSpecifier>,
  ) -> Result<Vec<PluginInfo>, AnyError> {
    let mut load_futures = Vec::with_capacity(plugin_specifiers.len());
    for specifier in plugin_specifiers {
      let mod_id = self
        .worker
        .js_runtime
        .load_side_es_module(&specifier)
        .await?;
      let mod_future = self.worker.js_runtime.mod_evaluate(mod_id).boxed_local();
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
      let module = self.worker.js_runtime.get_module_namespace(mod_id).unwrap();
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let module_local = v8::Local::new(scope, module);
      let default_export_str = DEFAULT.v8_string(scope).unwrap();
      let default_export = module_local.get(scope, default_export_str.into()).unwrap();
      let default_export_global = v8::Global::new(scope, default_export);
      plugin_handles.push(default_export_global);
    }

    deno_core::scope!(scope, &mut self.worker.js_runtime);
    let install_plugins_local =
      v8::Local::new(scope, &*self.install_plugins_fn.clone());
    let undefined = v8::undefined(scope);

    let local_handles = {
      let arr = v8::Array::new(scope, plugin_handles.len().try_into().unwrap());
      for (idx, plugin_handle) in plugin_handles.into_iter().enumerate() {
        let handle = v8::Local::new(scope, plugin_handle);
        arr
          .set_index(scope, idx.try_into().unwrap(), handle)
          .unwrap();
      }
      arr
    };
    let args = &[local_handles.into()];

    log::debug!("Installing vbundle plugins...");

    let plugins_info_result = {
      v8::tc_scope!(tc_scope, scope);
      let plugins_info_result =
        install_plugins_local.call(tc_scope, undefined.into(), args);
      if let Some(exception) = tc_scope.exception() {
        let error = JsError::from_v8_exception(tc_scope, exception);
        return Err(error.into());
      }
      plugins_info_result
    };
    let plugins_info = plugins_info_result.unwrap();
    let infos: Vec<PluginInfo> =
      deno_core::serde_v8::from_v8(scope, plugins_info)?;
    log::debug!("Plugins installed: {}", infos.len());

    Ok(infos)
  }

  /// Call plugin buildStart hooks.
  async fn build_start(&mut self) -> Result<(), AnyError> {
    let promise = {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let build_start_fn = v8::Local::new(scope, &*self.build_start_fn.clone());
      let undefined = v8::undefined(scope);

      let result = {
        v8::tc_scope!(tc_scope, scope);
        let result = build_start_fn.call(tc_scope, undefined.into(), &[]);
        if let Some(exception) = tc_scope.exception() {
          let error = JsError::from_v8_exception(tc_scope, exception);
          return Err(error.into());
        }
        result
      };

      result.map(|v| v8::Global::new(scope, v))
    };

    // Run event loop to handle any async operations
    self
      .worker
      .js_runtime
      .run_event_loop(PollEventLoopOptions::default())
      .await?;

    // Check if the promise resolved successfully
    if let Some(promise_global) = promise {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let promise_local = v8::Local::new(scope, &promise_global);
      if let Ok(promise) = v8::Local::<v8::Promise>::try_from(promise_local) {
        if promise.state() == v8::PromiseState::Rejected {
          let exception = promise.result(scope);
          let error = JsError::from_v8_exception(scope, exception);
          return Err(error.into());
        }
      }
    }

    Ok(())
  }

  /// Call plugin buildEnd hooks.
  async fn build_end(&mut self) -> Result<(), AnyError> {
    let promise = {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let build_end_fn = v8::Local::new(scope, &*self.build_end_fn.clone());
      let undefined = v8::undefined(scope);

      let result = {
        v8::tc_scope!(tc_scope, scope);
        let result = build_end_fn.call(tc_scope, undefined.into(), &[]);
        if let Some(exception) = tc_scope.exception() {
          let error = JsError::from_v8_exception(tc_scope, exception);
          return Err(error.into());
        }
        result
      };

      result.map(|v| v8::Global::new(scope, v))
    };

    self
      .worker
      .js_runtime
      .run_event_loop(PollEventLoopOptions::default())
      .await?;

    if let Some(promise_global) = promise {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let promise_local = v8::Local::new(scope, &promise_global);
      if let Ok(promise) = v8::Local::<v8::Promise>::try_from(promise_local) {
        if promise.state() == v8::PromiseState::Rejected {
          let exception = promise.result(scope);
          let error = JsError::from_v8_exception(scope, exception);
          return Err(error.into());
        }
      }
    }

    Ok(())
  }

  /// Call plugin resolveId hooks.
  async fn resolve_id(
    &mut self,
    source: &str,
    importer: Option<&str>,
    options: &ResolveOptions,
  ) -> Result<Option<ResolveResult>, AnyError> {
    let promise = {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let resolve_id_fn = v8::Local::new(scope, &*self.resolve_id_fn.clone());
      let undefined = v8::undefined(scope);

      let source_v8: v8::Local<v8::Value> =
        v8::String::new(scope, source).unwrap().into();
      let importer_v8: v8::Local<v8::Value> = match importer {
        Some(imp) => v8::String::new(scope, imp).unwrap().into(),
        None => v8::null(scope).into(),
      };
      let options_v8 = deno_core::serde_v8::to_v8(scope, options)?;

      let result = {
        v8::tc_scope!(tc_scope, scope);
        let result = resolve_id_fn.call(
          tc_scope,
          undefined.into(),
          &[source_v8, importer_v8, options_v8],
        );
        if let Some(exception) = tc_scope.exception() {
          let error = JsError::from_v8_exception(tc_scope, exception);
          return Err(error.into());
        }
        result
      };

      result.map(|v| v8::Global::new(scope, v))
    };

    self
      .worker
      .js_runtime
      .run_event_loop(PollEventLoopOptions::default())
      .await?;

    if let Some(promise_global) = promise {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let promise_local = v8::Local::new(scope, &promise_global);

      // Check if it's a promise and get resolved value
      let result_value = if let Ok(promise) =
        v8::Local::<v8::Promise>::try_from(promise_local)
      {
        if promise.state() == v8::PromiseState::Rejected {
          let exception = promise.result(scope);
          let error = JsError::from_v8_exception(scope, exception);
          return Err(error.into());
        }
        promise.result(scope)
      } else {
        promise_local
      };

      if result_value.is_null_or_undefined() {
        return Ok(None);
      }
      let resolve_result: ResolveResult =
        deno_core::serde_v8::from_v8(scope, result_value)?;
      return Ok(Some(resolve_result));
    }

    Ok(None)
  }

  /// Call plugin load hooks.
  async fn load(&mut self, id: &str) -> Result<Option<LoadResult>, AnyError> {
    let promise = {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let load_fn = v8::Local::new(scope, &*self.load_fn.clone());
      let undefined = v8::undefined(scope);

      let id_v8: v8::Local<v8::Value> = v8::String::new(scope, id).unwrap().into();

      let result = {
        v8::tc_scope!(tc_scope, scope);
        let result = load_fn.call(tc_scope, undefined.into(), &[id_v8]);
        if let Some(exception) = tc_scope.exception() {
          let error = JsError::from_v8_exception(tc_scope, exception);
          return Err(error.into());
        }
        result
      };

      result.map(|v| v8::Global::new(scope, v))
    };

    self
      .worker
      .js_runtime
      .run_event_loop(PollEventLoopOptions::default())
      .await?;

    if let Some(promise_global) = promise {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let promise_local = v8::Local::new(scope, &promise_global);

      let result_value = if let Ok(promise) =
        v8::Local::<v8::Promise>::try_from(promise_local)
      {
        if promise.state() == v8::PromiseState::Rejected {
          let exception = promise.result(scope);
          let error = JsError::from_v8_exception(scope, exception);
          return Err(error.into());
        }
        promise.result(scope)
      } else {
        promise_local
      };

      if result_value.is_null_or_undefined() {
        return Ok(None);
      }
      let load_result: LoadResult =
        deno_core::serde_v8::from_v8(scope, result_value)?;
      return Ok(Some(load_result));
    }

    Ok(None)
  }

  /// Call plugin transform hooks.
  async fn transform(
    &mut self,
    id: &str,
    code: &str,
  ) -> Result<Option<TransformResult>, AnyError> {
    let promise = {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let transform_fn = v8::Local::new(scope, &*self.transform_fn.clone());
      let undefined = v8::undefined(scope);

      let id_v8: v8::Local<v8::Value> = v8::String::new(scope, id).unwrap().into();
      let code_v8: v8::Local<v8::Value> =
        v8::String::new(scope, code).unwrap().into();

      let result = {
        v8::tc_scope!(tc_scope, scope);
        let result =
          transform_fn.call(tc_scope, undefined.into(), &[id_v8, code_v8]);
        if let Some(exception) = tc_scope.exception() {
          let error = JsError::from_v8_exception(tc_scope, exception);
          return Err(error.into());
        }
        result
      };

      result.map(|v| v8::Global::new(scope, v))
    };

    self
      .worker
      .js_runtime
      .run_event_loop(PollEventLoopOptions::default())
      .await?;

    if let Some(promise_global) = promise {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let promise_local = v8::Local::new(scope, &promise_global);

      let result_value = if let Ok(promise) =
        v8::Local::<v8::Promise>::try_from(promise_local)
      {
        if promise.state() == v8::PromiseState::Rejected {
          let exception = promise.result(scope);
          let error = JsError::from_v8_exception(scope, exception);
          return Err(error.into());
        }
        promise.result(scope)
      } else {
        promise_local
      };

      if result_value.is_null_or_undefined() {
        return Ok(None);
      }
      let transform_result: TransformResult =
        deno_core::serde_v8::from_v8(scope, result_value)?;
      return Ok(Some(transform_result));
    }

    Ok(None)
  }

  /// Call plugin renderChunk hooks.
  async fn render_chunk(
    &mut self,
    code: &str,
    chunk: &ChunkInfo,
  ) -> Result<Option<RenderChunkResult>, AnyError> {
    let promise = {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let render_chunk_fn = v8::Local::new(scope, &*self.render_chunk_fn.clone());
      let undefined = v8::undefined(scope);

      let code_v8: v8::Local<v8::Value> =
        v8::String::new(scope, code).unwrap().into();
      let chunk_v8 = deno_core::serde_v8::to_v8(scope, chunk)?;

      let result = {
        v8::tc_scope!(tc_scope, scope);
        let result =
          render_chunk_fn.call(tc_scope, undefined.into(), &[code_v8, chunk_v8]);
        if let Some(exception) = tc_scope.exception() {
          let error = JsError::from_v8_exception(tc_scope, exception);
          return Err(error.into());
        }
        result
      };

      result.map(|v| v8::Global::new(scope, v))
    };

    self
      .worker
      .js_runtime
      .run_event_loop(PollEventLoopOptions::default())
      .await?;

    if let Some(promise_global) = promise {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let promise_local = v8::Local::new(scope, &promise_global);

      let result_value = if let Ok(promise) =
        v8::Local::<v8::Promise>::try_from(promise_local)
      {
        if promise.state() == v8::PromiseState::Rejected {
          let exception = promise.result(scope);
          let error = JsError::from_v8_exception(scope, exception);
          return Err(error.into());
        }
        promise.result(scope)
      } else {
        promise_local
      };

      if result_value.is_null_or_undefined() {
        return Ok(None);
      }
      let render_result: RenderChunkResult =
        deno_core::serde_v8::from_v8(scope, result_value)?;
      return Ok(Some(render_result));
    }

    Ok(None)
  }

  /// Call plugin generateBundle hooks.
  async fn generate_bundle(
    &mut self,
    bundle: &std::collections::HashMap<String, ChunkInfo>,
  ) -> Result<(), AnyError> {
    let promise = {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let generate_bundle_fn =
        v8::Local::new(scope, &*self.generate_bundle_fn.clone());
      let undefined = v8::undefined(scope);

      let bundle_v8 = deno_core::serde_v8::to_v8(scope, bundle)?;

      let result = {
        v8::tc_scope!(tc_scope, scope);
        let result =
          generate_bundle_fn.call(tc_scope, undefined.into(), &[bundle_v8]);
        if let Some(exception) = tc_scope.exception() {
          let error = JsError::from_v8_exception(tc_scope, exception);
          return Err(error.into());
        }
        result
      };

      result.map(|v| v8::Global::new(scope, v))
    };

    self
      .worker
      .js_runtime
      .run_event_loop(PollEventLoopOptions::default())
      .await?;

    if let Some(promise_global) = promise {
      deno_core::scope!(scope, &mut self.worker.js_runtime);
      let promise_local = v8::Local::new(scope, &promise_global);
      if let Ok(promise) = v8::Local::<v8::Promise>::try_from(promise_local) {
        if promise.state() == v8::PromiseState::Rejected {
          let exception = promise.result(scope);
          let error = JsError::from_v8_exception(scope, exception);
          return Err(error.into());
        }
      }
    }

    Ok(())
  }
}

/// Create the plugin runner inner implementation.
async fn create_plugin_runner_inner(
  logger: PluginLogger,
  rx_req: mpsc::Receiver<PluginHostRequest>,
) -> Result<PluginHost, AnyError> {
  // Create flags for the plugin worker - plugins get file and env access
  // Use Vbundle subcommand to ensure vbundle JS files are loaded (needs_test() == true)
  let flags = Flags {
    subcommand: DenoSubcommand::Vbundle(crate::args::VbundleFlags::default()),
    permissions: PermissionFlags {
      allow_env: Some(vec![]),
      allow_read: Some(vec![]),
      allow_net: Some(vec![]),
      no_prompt: true,
      ..Default::default()
    },
    ..Default::default()
  };
  let flags = Arc::new(flags);
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let main_module = deno_path_util::resolve_url_or_path(
    "./$deno$vbundle.mts",
    cli_options.initial_cwd(),
  )
  .unwrap();
  let permissions = factory.root_permissions_container()?.clone();
  let worker_factory = factory.create_cli_main_worker_factory().await?;

  let worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Run,
      main_module.clone(),
      vec![],
      vec![],
      permissions,
      vec![crate::ops::vbundle::deno_vbundle_ext::init(logger.clone())],
      Default::default(),
      None,
    )
    .await?;

  let mut worker = worker.into_main_worker();
  let runtime = &mut worker.js_runtime;

  let obj = runtime.execute_script("vbundle.js", "Deno[Deno.internal]")?;

  log::debug!("Vbundle plugins loaded, capturing exports");
  let (
    install_plugins_fn,
    build_start_fn,
    build_end_fn,
    resolve_id_fn,
    load_fn,
    transform_fn,
    render_chunk_fn,
    generate_bundle_fn,
  ) = {
    deno_core::scope!(scope, runtime);
    let module_exports: v8::Local<v8::Object> =
      v8::Local::new(scope, obj).try_into().unwrap();

    let install_plugins_fn_name = INSTALL_PLUGINS.v8_string(scope).unwrap();
    let install_plugins_fn_val = module_exports
      .get(scope, install_plugins_fn_name.into())
      .unwrap();
    let install_plugins_fn: v8::Local<v8::Function> =
      install_plugins_fn_val.try_into().unwrap();

    let build_start_fn_name = BUILD_START.v8_string(scope).unwrap();
    let build_start_fn_val = module_exports
      .get(scope, build_start_fn_name.into())
      .unwrap();
    let build_start_fn: v8::Local<v8::Function> =
      build_start_fn_val.try_into().unwrap();

    let build_end_fn_name = BUILD_END.v8_string(scope).unwrap();
    let build_end_fn_val = module_exports
      .get(scope, build_end_fn_name.into())
      .unwrap();
    let build_end_fn: v8::Local<v8::Function> =
      build_end_fn_val.try_into().unwrap();

    let resolve_id_fn_name = RESOLVE_ID.v8_string(scope).unwrap();
    let resolve_id_fn_val = module_exports
      .get(scope, resolve_id_fn_name.into())
      .unwrap();
    let resolve_id_fn: v8::Local<v8::Function> =
      resolve_id_fn_val.try_into().unwrap();

    let load_fn_name = LOAD.v8_string(scope).unwrap();
    let load_fn_val = module_exports
      .get(scope, load_fn_name.into())
      .unwrap();
    let load_fn: v8::Local<v8::Function> =
      load_fn_val.try_into().unwrap();

    let transform_fn_name = TRANSFORM.v8_string(scope).unwrap();
    let transform_fn_val = module_exports
      .get(scope, transform_fn_name.into())
      .unwrap();
    let transform_fn: v8::Local<v8::Function> =
      transform_fn_val.try_into().unwrap();

    let render_chunk_fn_name = RENDER_CHUNK.v8_string(scope).unwrap();
    let render_chunk_fn_val = module_exports
      .get(scope, render_chunk_fn_name.into())
      .unwrap();
    let render_chunk_fn: v8::Local<v8::Function> =
      render_chunk_fn_val.try_into().unwrap();

    let generate_bundle_fn_name = GENERATE_BUNDLE.v8_string(scope).unwrap();
    let generate_bundle_fn_val = module_exports
      .get(scope, generate_bundle_fn_name.into())
      .unwrap();
    let generate_bundle_fn: v8::Local<v8::Function> =
      generate_bundle_fn_val.try_into().unwrap();

    (
      Rc::new(v8::Global::new(scope, install_plugins_fn)),
      Rc::new(v8::Global::new(scope, build_start_fn)),
      Rc::new(v8::Global::new(scope, build_end_fn)),
      Rc::new(v8::Global::new(scope, resolve_id_fn)),
      Rc::new(v8::Global::new(scope, load_fn)),
      Rc::new(v8::Global::new(scope, transform_fn)),
      Rc::new(v8::Global::new(scope, render_chunk_fn)),
      Rc::new(v8::Global::new(scope, generate_bundle_fn)),
    )
  };

  Ok(PluginHost {
    worker,
    install_plugins_fn,
    build_start_fn,
    build_end_fn,
    resolve_id_fn,
    load_fn,
    transform_fn,
    render_chunk_fn,
    generate_bundle_fn,
    rx: rx_req,
  })
}

/// Create a plugin host and load plugins.
pub async fn create_runner_and_load_plugins(
  plugin_specifiers: Vec<ModuleSpecifier>,
  logger: PluginLogger,
) -> Result<PluginHostProxy, AnyError> {
  let host_proxy = PluginHost::create(logger)?;
  if !plugin_specifiers.is_empty() {
    host_proxy.load_plugins(plugin_specifiers).await?;
  }
  Ok(host_proxy)
}
