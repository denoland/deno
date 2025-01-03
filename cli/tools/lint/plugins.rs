// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url_or_path;
use deno_core::v8;
use deno_core::PollEventLoopOptions;
use deno_lint::diagnostic::LintDiagnostic;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::tokio_util;
use deno_runtime::worker::MainWorker;
use deno_runtime::WorkerExecutionMode;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;

use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::LintFlags;
use crate::factory::CliFactory;
use crate::ops::lint::LintPluginContainer;
use crate::tools::lint::serialize_ast_to_buffer;

#[derive(Debug)]
pub enum PluginHostRequest {
  LoadPlugins(Vec<ModuleSpecifier>, Option<Vec<String>>),
  Run(Vec<u8>, PathBuf, SourceTextInfo),
}

pub enum PluginHostResponse {
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

#[derive(Clone)]
pub struct PluginLogger {
  print: fn(&str, bool),
  debug: bool,
}

impl PluginLogger {
  pub fn new(print: fn(&str, bool), debug: bool) -> Self {
    Self { print, debug }
  }

  pub fn log(&self, msg: &str) {
    (self.print)(msg, false);
  }

  pub fn error(&self, msg: &str) {
    (self.print)(msg, true);
  }

  pub fn debug(&self, msg: &str) {
    if self.debug {
      (self.print)(msg, false);
    }
  }
}

impl std::fmt::Debug for PluginLogger {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("PluginLogger").field(&self.debug).finish()
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
  INSTALL_PLUGIN = "installPlugin",
  RUN_PLUGINS_FOR_FILE = "runPluginsForFile",
}

#[derive(Debug)]
pub struct PluginHostProxy {
  tx: Sender<PluginHostRequest>,
  rx: Arc<tokio::sync::Mutex<Receiver<PluginHostResponse>>>,
  pub(crate) plugin_info: Arc<Mutex<Vec<PluginInfo>>>,
  #[allow(unused)]
  join_handle: std::thread::JoinHandle<Result<(), AnyError>>,
  logger: PluginLogger,
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
  install_plugin_fn: Rc<v8::Global<v8::Function>>,
  run_plugins_for_file_fn: Rc<v8::Global<v8::Function>>,
  tx: Sender<PluginHostResponse>,
  rx: Receiver<PluginHostRequest>,
  logger: PluginLogger,
}

async fn create_plugin_runner_inner(
  logger: PluginLogger,
  rx_req: Receiver<PluginHostRequest>,
  tx_res: Sender<PluginHostResponse>,
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
    &flags.permissions.to_options(&[]),
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

  logger.log("before loaded");

  let obj = runtime.execute_script("lint.js", "Deno[Deno.internal]")?;

  logger.log("After plugin loaded, capturing exports");
  let (install_plugin_fn, run_plugins_for_file_fn) = {
    let scope = &mut runtime.handle_scope();
    let module_exports: v8::Local<v8::Object> =
      v8::Local::new(scope, obj).try_into().unwrap();

    let install_plugin_fn_name = INSTALL_PLUGIN.v8_string(scope).unwrap();
    let install_plugin_fn_val = module_exports
      .get(scope, install_plugin_fn_name.into())
      .unwrap();
    let install_plugin_fn: v8::Local<v8::Function> =
      install_plugin_fn_val.try_into().unwrap();

    let run_plugins_for_file_fn_name =
      RUN_PLUGINS_FOR_FILE.v8_string(scope).unwrap();
    let run_plugins_for_file_fn_val = module_exports
      .get(scope, run_plugins_for_file_fn_name.into())
      .unwrap();
    let run_plugins_for_file_fn: v8::Local<v8::Function> =
      run_plugins_for_file_fn_val.try_into().unwrap();

    (
      Rc::new(v8::Global::new(scope, install_plugin_fn)),
      Rc::new(v8::Global::new(scope, run_plugins_for_file_fn)),
    )
  };

  Ok(PluginHost {
    worker,
    install_plugin_fn,
    run_plugins_for_file_fn,
    tx: tx_res,
    rx: rx_req,
    logger: logger.clone(),
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
    let (tx_req, rx_req) = channel(10);
    let (tx_res, rx_res) = channel(10);

    logger.log("spawning thread");
    let logger_ = logger.clone();
    let join_handle = std::thread::spawn(move || {
      let logger = logger_;
      logger.log("PluginHost thread spawned");
      let start = std::time::Instant::now();
      let fut = async move {
        let runner =
          create_plugin_runner_inner(logger.clone(), rx_req, tx_res).await?;
        // TODO(bartlomieju): send "host ready" message to the proxy
        logger.log("running host loop");
        runner.run_loop().await?;
        logger.log(&format!(
          "PluginHost thread finished, took {:?}",
          std::time::Instant::now() - start
        ));
        Ok(())
      }
      .boxed_local();
      tokio_util::create_and_run_current_thread(fut)
    });

    logger.log(&format!("is thread finished {}", join_handle.is_finished()));
    let proxy = PluginHostProxy {
      tx: tx_req,
      rx: Arc::new(tokio::sync::Mutex::new(rx_res)),
      plugin_info: Arc::new(Mutex::new(vec![])),
      join_handle,
      logger,
    };

    Ok(proxy)
  }

  async fn run_loop(mut self) -> Result<(), AnyError> {
    self.logger.log("waiting for message");
    while let Some(req) = self.rx.recv().await {
      self.logger.log("received message");
      match req {
        PluginHostRequest::LoadPlugins(specifiers, exclude) => {
          let r = self.load_plugins(specifiers, exclude).await;
          let _ = self.tx.send(PluginHostResponse::LoadPlugin(r)).await;
        }
        PluginHostRequest::Run(serialized_ast, specifier, source_text_info) => {
          let start = std::time::Instant::now();
          let r = match self
            .run_plugins(&specifier, serialized_ast, source_text_info)
            .await
          {
            Ok(()) => Ok(self.take_diagnostics()),
            Err(err) => Err(err),
          };
          self.logger.log(&format!(
            "Running rules took {:?}",
            std::time::Instant::now() - start
          ));
          let _ = self.tx.send(PluginHostResponse::Run(r)).await;
        }
      }
    }
    self.logger.log("breaking loop");
    Ok(())
  }

  fn take_diagnostics(&mut self) -> Vec<LintDiagnostic> {
    let op_state = self.worker.js_runtime.op_state();
    let mut state = op_state.borrow_mut();
    let container = state.borrow_mut::<LintPluginContainer>();
    std::mem::take(&mut container.diagnostics)
  }

  async fn run_plugins(
    &mut self,
    specifier: &Path,
    serialized_ast: Vec<u8>,
    source_text_info: SourceTextInfo,
  ) -> Result<(), AnyError> {
    {
      let state = self.worker.js_runtime.op_state();
      let mut state = state.borrow_mut();
      state.borrow_mut::<LintPluginContainer>().set_info_for_file(
        ModuleSpecifier::from_file_path(specifier).unwrap(),
        source_text_info,
      );
    }

    let (file_name_v8, ast_uint8arr_v8) = {
      let scope = &mut self.worker.js_runtime.handle_scope();
      let file_name_v8: v8::Local<v8::Value> =
        v8::String::new(scope, &specifier.display().to_string())
          .unwrap()
          .into();

      let store = v8::ArrayBuffer::new_backing_store_from_vec(serialized_ast);
      let ast_buf =
        v8::ArrayBuffer::with_backing_store(scope, &store.make_shared());
      let ast_bin_v8: v8::Local<v8::Value> =
        v8::Uint8Array::new(scope, ast_buf, 0, ast_buf.byte_length())
          .unwrap()
          .into();
      (
        v8::Global::new(scope, file_name_v8),
        v8::Global::new(scope, ast_bin_v8),
      )
    };

    let call = self.worker.js_runtime.call_with_args(
      &self.run_plugins_for_file_fn,
      &[file_name_v8, ast_uint8arr_v8],
    );
    let result = self
      .worker
      .js_runtime
      .with_event_loop_promise(call, PollEventLoopOptions::default())
      .await;
    match result {
      Ok(_r) => self.logger.log("plugins finished"),
      Err(error) => {
        self.logger.log(&format!("error running plugins {}", error));
      }
    }

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

    let mut infos = Vec::with_capacity(load_futures.len());

    for (fut, mod_id) in load_futures {
      fut.await?;
      let module = self.worker.js_runtime.get_module_namespace(mod_id).unwrap();
      let scope = &mut self.worker.js_runtime.handle_scope();
      let module_local = v8::Local::new(scope, module);
      let default_export_str = v8::String::new(scope, "default").unwrap();
      let default_export =
        module_local.get(scope, default_export_str.into()).unwrap();
      let install_plugins_local =
        v8::Local::new(scope, &*self.install_plugin_fn.clone());
      let undefined = v8::undefined(scope);

      let exclude_v8: v8::Local<v8::Value> =
        exclude.clone().map_or(v8::null(scope).into(), |v| {
          let elems = v
            .iter()
            .map(|item| v8::String::new(scope, item).unwrap().into())
            .collect::<Vec<_>>();

          v8::Array::new_with_elements(scope, elems.as_slice()).into()
        });

      let args = &[default_export, exclude_v8];

      self.logger.log("Installing plugin...");
      // TODO(bartlomieju): do it in a try/catch scope
      let plugin_info = install_plugins_local
        .call(scope, undefined.into(), args)
        .unwrap();
      let info: PluginInfo = deno_core::serde_v8::from_v8(scope, plugin_info)?;
      self.logger.log(&format!("Plugin installed: {}", info.name));
      infos.push(info);
    }

    Ok(infos)
  }
}

impl PluginHostProxy {
  pub async fn load_plugins(
    &self,
    plugin_specifiers: Vec<ModuleSpecifier>,
    exclude: Option<Vec<String>>,
  ) -> Result<(), AnyError> {
    self
      .tx
      .send(PluginHostRequest::LoadPlugins(plugin_specifiers, exclude))
      .await?;
    let mut rx = self.rx.lock().await;
    self.logger.log("receiving load plugins");
    if let Some(val) = rx.recv().await {
      let PluginHostResponse::LoadPlugin(result) = val else {
        unreachable!()
      };
      self
        .logger
        .error(&format!("load plugins response {:#?}", result));
      let infos = result?;
      *self.plugin_info.lock() = infos;
      return Ok(());
    }
    Err(custom_error("AlreadyClosed", "Plugin host has closed"))
  }

  pub async fn run_rules(
    &self,
    specifier: &Path,
    serialized_ast: Vec<u8>,
    source_text_info: SourceTextInfo,
  ) -> Result<Vec<LintDiagnostic>, AnyError> {
    self
      .tx
      .send(PluginHostRequest::Run(
        serialized_ast,
        specifier.to_path_buf(),
        source_text_info,
      ))
      .await?;
    let mut rx = self.rx.lock().await;
    self.logger.log("receiving diagnostics");
    if let Some(PluginHostResponse::Run(diagnostics_result)) = rx.recv().await {
      return diagnostics_result;
    }
    Err(custom_error("AlreadyClosed", "Plugin host has closed"))
  }

  pub fn serialize_ast(
    &self,
    parsed_source: ParsedSource,
  ) -> Result<Vec<u8>, AnyError> {
    let start = std::time::Instant::now();
    let r = serialize_ast_to_buffer(&parsed_source);
    self.logger.debug(&format!(
      "serialize custom ast took {:?}",
      std::time::Instant::now() - start
    ));
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
  host_proxy: &mut PluginHostProxy,
  specifier: &Path,
  serialized_ast: Vec<u8>,
  source_text_info: SourceTextInfo,
) -> Result<Vec<LintDiagnostic>, AnyError> {
  let d = host_proxy
    .run_rules(specifier, serialized_ast, source_text_info)
    .await?;
  Ok(d)
}
