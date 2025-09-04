// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use deno_config::glob::FilePatterns;
use deno_config::workspace::WorkspaceDirLintConfig;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_lint::linter::LintConfig;
use deno_resolver::deno_json::CompilerOptionsKey;
use deno_runtime::tokio_util::create_basic_runtime;
use once_cell::sync::Lazy;

use crate::args::LintFlags;
use crate::args::LintOptions;
use crate::lsp::compiler_options::LspCompilerOptionsResolver;
use crate::lsp::config::Config;
use crate::lsp::documents::DocumentModule;
use crate::lsp::logging::lsp_log;
use crate::lsp::logging::lsp_warn;
use crate::lsp::resolver::LspResolver;
use crate::tools::lint::CliLinter;
use crate::tools::lint::CliLinterOptions;
use crate::tools::lint::LintRuleProvider;
use crate::tools::lint::PluginHostProxy;

#[derive(Debug)]
pub struct LspLinter {
  pub inner: CliLinter,
  pub lint_config: WorkspaceDirLintConfig,
}

#[derive(Debug, Default)]
pub struct LspLinterResolver {
  config: Config,
  compiler_options_resolver: Arc<LspCompilerOptionsResolver>,
  resolver: Arc<LspResolver>,
  linters: DashMap<(CompilerOptionsKey, Option<Arc<Url>>), Arc<LspLinter>>,
}

impl LspLinterResolver {
  pub fn new(
    config: &Config,
    compiler_options_resolver: &Arc<LspCompilerOptionsResolver>,
    resolver: &Arc<LspResolver>,
  ) -> Self {
    Self {
      config: config.clone(),
      compiler_options_resolver: compiler_options_resolver.clone(),
      resolver: resolver.clone(),
      linters: Default::default(),
    }
  }

  pub fn for_module(&self, module: &DocumentModule) -> Arc<LspLinter> {
    self
      .linters
      .entry((module.compiler_options_key.clone(), module.scope.clone()))
      .or_insert_with(|| {
        let config_data = module
          .scope
          .as_ref()
          .and_then(|s| self.config.tree.data_for_specifier(s));
        let workspace_resolver = self
          .resolver
          .get_scoped_resolver(config_data.map(|d| d.scope.as_ref()))
          .as_workspace_resolver()
          .clone();
        let lint_rule_provider =
          LintRuleProvider::new(Some(workspace_resolver));
        let lint_config = config_data
          .and_then(|d| {
            d.member_dir
              .to_lint_config(FilePatterns::new_with_base(
                d.member_dir.dir_path(),
              ))
              .inspect_err(|err| {
                lsp_warn!("Couldn't read lint configuration: {}", err)
              })
              .ok()
          })
          .unwrap_or_else(|| WorkspaceDirLintConfig {
            rules: Default::default(),
            plugins: Default::default(),
            files: FilePatterns::new_with_base(PathBuf::from("/")),
          });
        let lint_options =
          LintOptions::resolve(lint_config.clone(), &LintFlags::default())
            .inspect_err(|err| {
              lsp_warn!("Failed to resolve linter options: {}", err)
            })
            .ok()
            .unwrap_or_default();
        let compiler_options_data = self
          .compiler_options_resolver
          .for_key(&module.compiler_options_key)
          .expect("Key should be in sync with resolver.");
        let deno_lint_config = if compiler_options_data
          .compiler_options
          .0
          .get("jsx")
          .and_then(|v| v.as_str())
          == Some("react")
        {
          let default_jsx_factory = compiler_options_data
            .compiler_options
            .0
            .get("jsxFactory")
            .and_then(|v| v.as_str());
          let default_jsx_fragment_factory = compiler_options_data
            .compiler_options
            .0
            .get("jsxFragmentFactory")
            .and_then(|v| v.as_str());
          LintConfig {
            default_jsx_factory: default_jsx_factory.map(String::from),
            default_jsx_fragment_factory: default_jsx_fragment_factory
              .map(String::from),
          }
        } else {
          LintConfig {
            default_jsx_factory: None,
            default_jsx_fragment_factory: None,
          }
        };
        let mut plugin_runner = None;
        if !lint_options.plugins.is_empty() {
          let load_plugins_result = LOAD_PLUGINS_THREAD.load_plugins(
            lint_options.plugins.clone(),
            lint_options.rules.exclude.clone(),
          );
          match load_plugins_result {
            Ok(runner) => {
              plugin_runner = Some(Arc::new(runner));
            }
            Err(err) => {
              lsp_warn!("Failed to load lint plugins: {}", err);
            }
          }
        }
        let inner = CliLinter::new(CliLinterOptions {
          configured_rules: lint_rule_provider.resolve_lint_rules(
            lint_options.rules,
            config_data.map(|d| d.member_dir.as_ref()),
          ),
          fix: false,
          deno_lint_config,
          maybe_plugin_runner: plugin_runner,
        });
        Arc::new(LspLinter { inner, lint_config })
      })
      .clone()
  }
}

#[derive(Debug)]
struct LoadPluginsRequest {
  plugins: Vec<Url>,
  exclude: Option<Vec<String>>,
  response_tx: std::sync::mpsc::Sender<Result<PluginHostProxy, AnyError>>,
}

#[derive(Debug)]
struct LoadPluginsThread {
  join_handle: Option<std::thread::JoinHandle<()>>,
  request_tx: Option<tokio::sync::mpsc::UnboundedSender<LoadPluginsRequest>>,
}

impl LoadPluginsThread {
  fn create() -> Self {
    let (request_tx, mut request_rx) =
      tokio::sync::mpsc::unbounded_channel::<LoadPluginsRequest>();
    let join_handle = std::thread::spawn(move || {
      create_basic_runtime().block_on(async move {
        while let Some(request) = request_rx.recv().await {
          let result = crate::tools::lint::create_runner_and_load_plugins(
            request.plugins,
            crate::tools::lint::PluginLogger::new(|msg, _is_err| {
              lsp_log!("pluggin runner - {}", msg);
            }),
            request.exclude,
          )
          .await;
          request.response_tx.send(result).unwrap();
        }
      });
    });
    Self {
      join_handle: Some(join_handle),
      request_tx: Some(request_tx),
    }
  }

  fn load_plugins(
    &self,
    plugins: Vec<Url>,
    exclude: Option<Vec<String>>,
  ) -> Result<PluginHostProxy, AnyError> {
    let request_tx = self.request_tx.as_ref().unwrap();
    let (response_tx, response_rx) = std::sync::mpsc::channel();
    let _ = request_tx.send(LoadPluginsRequest {
      plugins,
      exclude,
      response_tx,
    });
    response_rx.recv().unwrap()
  }
}

impl Drop for LoadPluginsThread {
  fn drop(&mut self) {
    drop(self.request_tx.take());
    self.join_handle.take().unwrap().join().unwrap();
  }
}

static LOAD_PLUGINS_THREAD: Lazy<LoadPluginsThread> =
  Lazy::new(LoadPluginsThread::create);
