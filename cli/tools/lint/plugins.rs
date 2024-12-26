// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::resolve_url_or_path;
use deno_core::v8;
use deno_core::PollEventLoopOptions;
use deno_lint::diagnostic::LintDiagnostic;
use deno_runtime::deno_io::Stdio;
use deno_runtime::deno_io::StdioPipe;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::tokio_util;
use deno_runtime::worker::MainWorker;
use deno_runtime::WorkerExecutionMode;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
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
pub enum PluginRunnerRequest {
  LoadPlugins(Vec<ModuleSpecifier>),
  Run(Vec<u8>, PathBuf, SourceTextInfo),
}

pub enum PluginRunnerResponse {
  LoadPlugin(Result<(), AnyError>),
  Run(Result<Vec<LintDiagnostic>, AnyError>),
}

impl std::fmt::Debug for PluginRunnerResponse {
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

#[derive(Debug)]
pub struct PluginRunnerProxy {
  tx: Sender<PluginRunnerRequest>,
  rx: Arc<tokio::sync::Mutex<Receiver<PluginRunnerResponse>>>,
  #[allow(unused)]
  join_handle: std::thread::JoinHandle<Result<(), AnyError>>,
  logger: PluginLogger,
}

pub struct PluginRunner {
  worker: MainWorker,
  install_plugin_fn: v8::Global<v8::Function>,
  run_plugins_for_file_fn: v8::Global<v8::Function>,
  tx: Sender<PluginRunnerResponse>,
  rx: Receiver<PluginRunnerRequest>,
  logger: PluginLogger,
}

impl PluginRunner {
  fn create(logger: PluginLogger) -> Result<PluginRunnerProxy, AnyError> {
    let (tx_req, rx_req) = channel(10);
    let (tx_res, rx_res) = channel(10);

    logger.log("spawning thread");
    let logger_ = logger.clone();
    let join_handle = std::thread::spawn(move || {
      let logger = logger_;
      logger.log("PluginRunner thread spawned");
      let start = std::time::Instant::now();
      let fut = async move {
        let flags = Flags {
          subcommand: DenoSubcommand::Lint(LintFlags::default()),
          ..Default::default()
        };
        let flags = Arc::new(flags);
        let factory = CliFactory::from_flags(flags);
        let cli_options = factory.cli_options()?;
        let main_module =
          resolve_url_or_path("./$deno$lint.mts", cli_options.initial_cwd())
            .unwrap();
        // TODO(bartlomieju): should we run with all permissions?
        let permissions = PermissionsContainer::allow_all(
          factory.permission_desc_parser()?.clone(),
          // Permissions::none(false),
        );
        // let npm_resolver = factory.npm_resolver().await?.clone();
        // let resolver = factory.resolver().await?.clone();
        let worker_factory = factory.create_cli_main_worker_factory().await?;

        let dev_null = std::fs::File::open("/dev/null").unwrap();
        let dev_null2 = std::fs::File::open("/dev/null").unwrap();

        let worker = worker_factory
          .create_custom_worker(
            // TODO(bartlomieju): add "lint" execution mode
            WorkerExecutionMode::Run,
            main_module.clone(),
            permissions,
            vec![crate::ops::lint::deno_lint_ext::init_ops(logger.clone())],
            Stdio {
              stdin: StdioPipe::inherit(),
              stdout: StdioPipe::file(dev_null),
              stderr: StdioPipe::file(dev_null2),
            },
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

          // TODO(bartlomieju): use v8::OneByteConst and `v8_static_strings!` macro from `deno_core`.
          let install_plugin_fn_name =
            v8::String::new(scope, "installPlugin").unwrap();
          let install_plugin_fn_val = module_exports
            .get(scope, install_plugin_fn_name.into())
            .unwrap();
          let install_plugin_fn: v8::Local<v8::Function> =
            install_plugin_fn_val.try_into().unwrap();

          // TODO(bartlomieju): use v8::OneByteConst and `v8_static_strings!` macro from `deno_core`.
          let run_plugins_for_file_fn_name =
            v8::String::new(scope, "runPluginsForFile").unwrap();
          let run_plugins_for_file_fn_val = module_exports
            .get(scope, run_plugins_for_file_fn_name.into())
            .unwrap();
          let run_plugins_for_file_fn: v8::Local<v8::Function> =
            run_plugins_for_file_fn_val.try_into().unwrap();

          (
            v8::Global::new(scope, install_plugin_fn),
            v8::Global::new(scope, run_plugins_for_file_fn),
          )
        };

        let runner = Self {
          worker,
          install_plugin_fn,
          run_plugins_for_file_fn,
          tx: tx_res,
          rx: rx_req,
          logger: logger.clone(),
        };
        // TODO(bartlomieju): send "host ready" message to the proxy
        logger.log("running host loop");
        runner.run_loop().await?;
        logger.log(&format!(
          "PluginRunner thread finished, took {:?}",
          std::time::Instant::now() - start
        ));
        Ok(())
      }
      .boxed_local();
      tokio_util::create_and_run_current_thread(fut)
    });

    logger.log(&format!("is thread finished {}", join_handle.is_finished()));
    let proxy = PluginRunnerProxy {
      tx: tx_req,
      rx: Arc::new(tokio::sync::Mutex::new(rx_res)),
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
        PluginRunnerRequest::LoadPlugins(specifiers) => {
          let r = self.load_plugins(specifiers).await;
          let _ = self.tx.send(PluginRunnerResponse::LoadPlugin(r)).await;
        }
        PluginRunnerRequest::Run(
          serialized_ast,
          specifier,
          source_text_info,
        ) => {
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
          let _ = self.tx.send(PluginRunnerResponse::Run(r)).await;
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
      let container = state.borrow_mut::<LintPluginContainer>();
      container.source_text_info = Some(source_text_info);
      container.specifier =
        Some(ModuleSpecifier::from_file_path(specifier).unwrap());
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
  ) -> Result<(), AnyError> {
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

    for (fut, mod_id) in load_futures {
      fut.await?;
      let module = self.worker.js_runtime.get_module_namespace(mod_id).unwrap();
      let scope = &mut self.worker.js_runtime.handle_scope();
      let module_local = v8::Local::new(scope, module);
      let default_export_str = v8::String::new(scope, "default").unwrap();
      let default_export =
        module_local.get(scope, default_export_str.into()).unwrap();
      // TODO(bartlomieju): put `install_plugin_fn` behind na `Rc``
      let install_plugins_local =
        v8::Local::new(scope, self.install_plugin_fn.clone());
      let undefined = v8::undefined(scope);
      let args = &[default_export];
      self.logger.log("Installing plugin...");
      // TODO(bartlomieju): do it in a try/catch scope
      install_plugins_local.call(scope, undefined.into(), args);
      self.logger.log("Plugin installed");
    }

    Ok(())
  }
}

impl PluginRunnerProxy {
  pub async fn load_plugins(
    &self,
    plugin_specifiers: Vec<ModuleSpecifier>,
  ) -> Result<(), AnyError> {
    self
      .tx
      .send(PluginRunnerRequest::LoadPlugins(plugin_specifiers))
      .await?;
    let mut rx = self.rx.lock().await;
    self.logger.log("receiving load plugins");
    if let Some(val) = rx.recv().await {
      let PluginRunnerResponse::LoadPlugin(result) = val else {
        unreachable!()
      };
      self
        .logger
        .error(&format!("load plugins response {:#?}", result));
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
      .send(PluginRunnerRequest::Run(
        serialized_ast,
        specifier.to_path_buf(),
        source_text_info,
      ))
      .await?;
    let mut rx = self.rx.lock().await;
    self.logger.log("receiving diagnostics");
    if let Some(PluginRunnerResponse::Run(diagnostics_result)) = rx.recv().await
    {
      return diagnostics_result;
    }
    Err(custom_error("AlreadyClosed", "Plugin host has closed"))
  }
}

pub async fn create_runner_and_load_plugins(
  plugin_specifiers: Vec<ModuleSpecifier>,
  logger: PluginLogger,
) -> Result<PluginRunnerProxy, AnyError> {
  let runner_proxy = PluginRunner::create(logger)?;
  runner_proxy.load_plugins(plugin_specifiers).await?;
  Ok(runner_proxy)
}

pub async fn run_rules_for_ast(
  runner_proxy: &mut PluginRunnerProxy,
  specifier: &Path,
  serialized_ast: Vec<u8>,
  source_text_info: SourceTextInfo,
) -> Result<Vec<LintDiagnostic>, AnyError> {
  let d = runner_proxy
    .run_rules(specifier, serialized_ast, source_text_info)
    .await?;
  Ok(d)
}

pub fn serialize_ast(parsed_source: ParsedSource) -> Result<Vec<u8>, AnyError> {
  let start = std::time::Instant::now();
  let r = serialize_ast_to_buffer(&parsed_source);
  // log::info!(
  //   "serialize custom ast took {:?}",
  //   std::time::Instant::now() - start
  // );
  Ok(r)
}
