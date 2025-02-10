// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use ::tokio_util::sync::CancellationToken;
use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::error::CoreError;
use deno_core::error::JsError;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url_or_path;
use deno_core::v8;
use deno_core::PollEventLoopOptions;
use deno_lint::diagnostic::LintDiagnostic;
use deno_path_util::url_from_file_path;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::tokio_util;
use deno_runtime::worker::MainWorker;
use deno_runtime::WorkerExecutionMode;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::LintFlags;
use crate::factory::CliFactory;
use crate::ops::lint::LintPluginContainer;
use crate::tools::lint::serialize_ast_to_buffer;
use crate::util::text_encoding::Utf16Map;

#[derive(Debug)]
pub enum PluginHostRequest {
  LoadPlugins {
    specifiers: Vec<ModuleSpecifier>,
    exclude_rules: Option<Vec<String>>,
    tx: oneshot::Sender<PluginHostResponse>,
  },
  Run {
    serialized_ast: Vec<u8>,
    file_path: PathBuf,
    source_text_info: SourceTextInfo,
    utf16_map: Utf16Map,
    maybe_token: Option<CancellationToken>,
    tx: oneshot::Sender<PluginHostResponse>,
  },
}

pub enum PluginHostResponse {
  // TODO: write to structs
  LoadPlugin(Result<Vec<PluginInfo>, AnyError>),
  Run(Result<Vec<LintDiagnostic>, AnyError>),
}

impl std::fmt::Debug for PluginHostResponse {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::LoadPlugin(_arg0) => f.debug_tuple("LoadPlugin").finish(),
      Self::Run(_arg0) => f.debug_tuple("Run").finish(),
    }
  }
}

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
  RUN_PLUGINS_FOR_FILE = "runPluginsForFile",
}

#[derive(Debug)]
pub struct PluginHostProxy {
  tx: mpsc::Sender<PluginHostRequest>,
  pub(crate) plugin_info: Arc<Mutex<Vec<PluginInfo>>>,
  #[allow(unused)]
  join_handle: std::thread::JoinHandle<Result<(), AnyError>>,
}

impl PluginHostProxy {
  pub fn get_plugin_rules(&self) -> Vec<String> {
    let infos = self.plugin_info.lock();

    let mut all_names = vec![];

    for info in infos.iter() {
      all_names.extend_from_slice(&info.get_rules());
    }

    all_names
  }
}

pub struct PluginHost {
  worker: MainWorker,
  install_plugins_fn: Rc<v8::Global<v8::Function>>,
  run_plugins_for_file_fn: Rc<v8::Global<v8::Function>>,
  rx: mpsc::Receiver<PluginHostRequest>,
}

async fn create_plugin_runner_inner(
  logger: PluginLogger,
  rx_req: mpsc::Receiver<PluginHostRequest>,
) -> Result<PluginHost, AnyError> {
  let flags = Flags {
    subcommand: DenoSubcommand::Lint(LintFlags::default()),
    ..Default::default()
  };
  let flags = Arc::new(flags);
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let main_module =
    resolve_url_or_path("./$deno$lint.mts", cli_options.initial_cwd()).unwrap();
  let perm_parser = factory.permission_desc_parser()?;
  let permissions = Permissions::from_options(
    perm_parser.as_ref(),
    &cli_options.permissions_options(),
  )?;
  let permissions = PermissionsContainer::new(perm_parser.clone(), permissions);
  // let npm_resolver = factory.npm_resolver().await?.clone();
  // let resolver = factory.resolver().await?.clone();
  let worker_factory = factory.create_cli_main_worker_factory().await?;

  let worker = worker_factory
    .create_custom_worker(
      // TODO(bartlomieju): add "lint" execution mode
      WorkerExecutionMode::Run,
      main_module.clone(),
      permissions,
      vec![crate::ops::lint::deno_lint_ext::init_ops(logger.clone())],
      Default::default(),
    )
    .await?;

  let mut worker = worker.into_main_worker();
  let runtime = &mut worker.js_runtime;

  let obj = runtime.execute_script("lint.js", "Deno[Deno.internal]")?;

  log::debug!("Lint plugins loaded, capturing default exports");
  let (install_plugins_fn, run_plugins_for_file_fn) = {
    let scope = &mut runtime.handle_scope();
    let module_exports: v8::Local<v8::Object> =
      v8::Local::new(scope, obj).try_into().unwrap();

    let install_plugins_fn_name = INSTALL_PLUGINS.v8_string(scope).unwrap();
    let install_plugins_fn_val = module_exports
      .get(scope, install_plugins_fn_name.into())
      .unwrap();
    let install_plugins_fn: v8::Local<v8::Function> =
      install_plugins_fn_val.try_into().unwrap();

    let run_plugins_for_file_fn_name =
      RUN_PLUGINS_FOR_FILE.v8_string(scope).unwrap();
    let run_plugins_for_file_fn_val = module_exports
      .get(scope, run_plugins_for_file_fn_name.into())
      .unwrap();
    let run_plugins_for_file_fn: v8::Local<v8::Function> =
      run_plugins_for_file_fn_val.try_into().unwrap();

    (
      Rc::new(v8::Global::new(scope, install_plugins_fn)),
      Rc::new(v8::Global::new(scope, run_plugins_for_file_fn)),
    )
  };

  Ok(PluginHost {
    worker,
    install_plugins_fn,
    run_plugins_for_file_fn,
    rx: rx_req,
  })
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
  pub name: String,
  pub rule_names: Vec<String>,
}

impl PluginInfo {
  pub fn get_rules(&self) -> Vec<String> {
    let mut rules = Vec::with_capacity(self.rule_names.len());

    for rule_name in &self.rule_names {
      rules.push(format!("{}/{}", self.name, rule_name));
    }

    rules
  }
}

impl PluginHost {
  fn create(logger: PluginLogger) -> Result<PluginHostProxy, AnyError> {
    let (tx_req, rx_req) = mpsc::channel(10);

    let logger_ = logger.clone();
    let join_handle = std::thread::spawn(move || {
      let logger = logger_;
      log::debug!("Lint PluginHost thread spawned");
      let start = std::time::Instant::now();
      let fut = async move {
        let runner = create_plugin_runner_inner(logger.clone(), rx_req).await?;
        log::debug!("Lint PlugibnHost running loop");
        runner.run_loop().await?;
        log::debug!(
          "Lint PluginHost thread finished, took {:?}",
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

  async fn run_loop(mut self) -> Result<(), AnyError> {
    log::debug!("Lint PluginHost is waiting for message");
    while let Some(req) = self.rx.recv().await {
      log::debug!("Lint PluginHost has received a message");
      match req {
        PluginHostRequest::LoadPlugins {
          specifiers,
          exclude_rules,
          tx,
        } => {
          let r = self.load_plugins(specifiers, exclude_rules).await;
          let _ = tx.send(PluginHostResponse::LoadPlugin(r));
        }
        PluginHostRequest::Run {
          serialized_ast,
          file_path,
          source_text_info,
          utf16_map,
          maybe_token,
          tx,
        } => {
          let start = std::time::Instant::now();
          let r = match self.run_plugins(
            &file_path,
            serialized_ast,
            source_text_info,
            utf16_map,
            maybe_token,
          ) {
            Ok(()) => Ok(self.take_diagnostics()),
            Err(err) => Err(err),
          };
          log::debug!(
            "Running plugins lint rules took {:?}",
            std::time::Instant::now() - start
          );
          let _ = tx.send(PluginHostResponse::Run(r));
        }
      }
    }
    log::debug!("Lint PluginHost run loop finished");
    Ok(())
  }

  fn take_diagnostics(&mut self) -> Vec<LintDiagnostic> {
    let op_state = self.worker.js_runtime.op_state();
    let mut state = op_state.borrow_mut();
    let container = state.borrow_mut::<LintPluginContainer>();
    std::mem::take(&mut container.diagnostics)
  }

  fn run_plugins(
    &mut self,
    file_path: &Path,
    serialized_ast: Vec<u8>,
    source_text_info: SourceTextInfo,
    utf16_map: Utf16Map,
    maybe_token: Option<CancellationToken>,
  ) -> Result<(), AnyError> {
    {
      let state = self.worker.js_runtime.op_state();
      let mut state = state.borrow_mut();
      let container = state.borrow_mut::<LintPluginContainer>();
      container.set_info_for_file(
        url_from_file_path(file_path)?,
        source_text_info,
        utf16_map,
        maybe_token,
      );
    }

    let scope = &mut self.worker.js_runtime.handle_scope();
    let file_name_v8: v8::Local<v8::Value> =
      v8::String::new(scope, &file_path.display().to_string())
        .unwrap()
        .into();

    let store = v8::ArrayBuffer::new_backing_store_from_vec(serialized_ast);
    let ast_buf =
      v8::ArrayBuffer::with_backing_store(scope, &store.make_shared());
    let ast_bin_v8: v8::Local<v8::Value> =
      v8::Uint8Array::new(scope, ast_buf, 0, ast_buf.byte_length())
        .unwrap()
        .into();
    let run_plugins_for_file =
      v8::Local::new(scope, &*self.run_plugins_for_file_fn);
    let undefined = v8::undefined(scope);

    let mut tc_scope = v8::TryCatch::new(scope);
    let _run_plugins_result = run_plugins_for_file.call(
      &mut tc_scope,
      undefined.into(),
      &[file_name_v8, ast_bin_v8],
    );

    if let Some(exception) = tc_scope.exception() {
      let error = JsError::from_v8_exception(&mut tc_scope, exception);
      let core_err = CoreError::Js(error);
      return Err(core_err.into());
    }
    drop(tc_scope);
    Ok(())
  }

  async fn load_plugins(
    &mut self,
    plugin_specifiers: Vec<ModuleSpecifier>,
    exclude: Option<Vec<String>>,
  ) -> Result<Vec<PluginInfo>, AnyError> {
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
      let module = self.worker.js_runtime.get_module_namespace(mod_id).unwrap();
      let scope = &mut self.worker.js_runtime.handle_scope();
      let module_local = v8::Local::new(scope, module);
      let default_export_str = DEFAULT.v8_string(scope).unwrap();
      let default_export =
        module_local.get(scope, default_export_str.into()).unwrap();
      let default_export_global = v8::Global::new(scope, default_export);
      plugin_handles.push(default_export_global);
    }

    let scope = &mut self.worker.js_runtime.handle_scope();
    let install_plugins_local =
      v8::Local::new(scope, &*self.install_plugins_fn.clone());
    let exclude_v8: v8::Local<v8::Value> =
      exclude.map_or(v8::null(scope).into(), |v| {
        let elems = v
          .iter()
          .map(|item| v8::String::new(scope, item).unwrap().into())
          .collect::<Vec<_>>();

        v8::Array::new_with_elements(scope, elems.as_slice()).into()
      });

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
    let args = &[local_handles.into(), exclude_v8];

    log::debug!("Installing lint plugins...");

    let mut tc_scope = v8::TryCatch::new(scope);
    let plugins_info_result =
      install_plugins_local.call(&mut tc_scope, undefined.into(), args);
    if let Some(exception) = tc_scope.exception() {
      let error = JsError::from_v8_exception(&mut tc_scope, exception);
      return Err(error.into());
    }
    drop(tc_scope);
    let plugins_info = plugins_info_result.unwrap();
    let infos: Vec<PluginInfo> =
      deno_core::serde_v8::from_v8(scope, plugins_info)?;
    log::debug!("Plugins installed: {}", infos.len());

    Ok(infos)
  }
}

impl PluginHostProxy {
  pub async fn load_plugins(
    &self,
    specifiers: Vec<ModuleSpecifier>,
    exclude_rules: Option<Vec<String>>,
  ) -> Result<(), AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::LoadPlugins {
        specifiers,
        exclude_rules,
        tx,
      })
      .await?;

    if let Ok(val) = rx.await {
      let PluginHostResponse::LoadPlugin(result) = val else {
        unreachable!()
      };
      let infos = result?;
      *self.plugin_info.lock() = infos;
      return Ok(());
    }
    bail!("Plugin host has closed")
  }

  pub async fn run_rules(
    &self,
    specifier: &Path,
    serialized_ast: Vec<u8>,
    source_text_info: SourceTextInfo,
    utf16_map: Utf16Map,
    maybe_token: Option<CancellationToken>,
  ) -> Result<Vec<LintDiagnostic>, AnyError> {
    let (tx, rx) = oneshot::channel();
    self
      .tx
      .send(PluginHostRequest::Run {
        serialized_ast,
        file_path: specifier.to_path_buf(),
        source_text_info,
        utf16_map,
        maybe_token,
        tx,
      })
      .await?;

    if let Ok(PluginHostResponse::Run(diagnostics_result)) = rx.await {
      return diagnostics_result;
    }
    bail!("Plugin host has closed")
  }

  pub fn serialize_ast(
    &self,
    parsed_source: &ParsedSource,
    utf16_map: &Utf16Map,
  ) -> Result<Vec<u8>, AnyError> {
    let start = std::time::Instant::now();
    let r = serialize_ast_to_buffer(parsed_source, utf16_map);
    log::debug!(
      "Serializing an AST took {:?}",
      std::time::Instant::now() - start
    );
    Ok(r)
  }
}

pub async fn create_runner_and_load_plugins(
  plugin_specifiers: Vec<ModuleSpecifier>,
  logger: PluginLogger,
  exclude: Option<Vec<String>>,
) -> Result<PluginHostProxy, AnyError> {
  let host_proxy = PluginHost::create(logger)?;
  host_proxy.load_plugins(plugin_specifiers, exclude).await?;
  Ok(host_proxy)
}

pub async fn run_rules_for_ast(
  host_proxy: &PluginHostProxy,
  specifier: &Path,
  serialized_ast: Vec<u8>,
  source_text_info: SourceTextInfo,
  utf16_map: Utf16Map,
  maybe_token: Option<CancellationToken>,
) -> Result<Vec<LintDiagnostic>, AnyError> {
  let d = host_proxy
    .run_rules(
      specifier,
      serialized_ast,
      source_text_info,
      utf16_map,
      maybe_token,
    )
    .await?;
  Ok(d)
}
