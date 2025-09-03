// Copyright 2018-2025 the Deno authors. MIT license.

mod esbuild;
mod externals;
mod provider;
mod transform;

use std::borrow::Cow;
use std::cell::RefCell;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;

use deno_ast::EmitOptions;
use deno_ast::MediaType;
use deno_ast::ModuleKind;
use deno_ast::ModuleSpecifier;
use deno_config::workspace::TsTypeLib;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt as _;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_error::JsError;
use deno_graph::ModuleErrorKind;
use deno_graph::Position;
use deno_path_util::resolve_url_or_path;
use deno_resolver::cache::ParsedSourceCache;
use deno_resolver::graph::ResolveWithGraphError;
use deno_resolver::graph::ResolveWithGraphOptions;
use deno_resolver::loader::LoadCodeSourceError;
use deno_resolver::loader::LoadCodeSourceErrorKind;
use deno_resolver::loader::LoadPreparedModuleErrorKind;
use deno_resolver::loader::LoadedModuleOrAsset;
use deno_resolver::loader::LoadedModuleSource;
use deno_resolver::loader::RequestedModuleType;
use deno_resolver::npm::managed::ResolvePkgFolderFromDenoModuleError;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageReqReference;
use esbuild_client::EsbuildFlags;
use esbuild_client::EsbuildFlagsBuilder;
use esbuild_client::EsbuildService;
use esbuild_client::protocol;
use esbuild_client::protocol::BuildResponse;
use indexmap::IndexMap;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::errors::PackageSubpathResolveError;
pub use provider::CliBundleProvider;
use sys_traits::EnvCurrentDir;

use crate::args::BundleFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::file_fetcher::CliFileFetcher;
use crate::graph_container::MainModuleGraphContainer;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::module_loader::CliDenoResolverModuleLoader;
use crate::module_loader::CliEmitter;
use crate::module_loader::ModuleLoadPreparer;
use crate::module_loader::PrepareModuleLoadOptions;
use crate::node::CliNodeResolver;
use crate::npm::CliNpmResolver;
use crate::resolver::CliCjsTracker;
use crate::resolver::CliResolver;
use crate::sys::CliSys;
use crate::tools::bundle::externals::ExternalsMatcher;
use crate::util::file_watcher::WatcherRestartMode;
use deno_bundle_runtime::BundleFormat;
use deno_bundle_runtime::BundlePlatform;
use deno_bundle_runtime::PackageHandling;
use deno_bundle_runtime::SourceMapType;

static DISABLE_HACK: LazyLock<bool> =
  LazyLock::new(|| std::env::var("NO_DENO_BUNDLE_HACK").is_err());

pub async fn bundle_init(
  mut flags: Arc<Flags>,
  bundle_flags: &BundleFlags,
) -> Result<EsbuildBundler, AnyError> {
  {
    let flags_mut = Arc::make_mut(&mut flags);
    flags_mut.unstable_config.sloppy_imports = true;
  }
  let factory = CliFactory::from_flags(flags.clone());

  let esbuild_path = ensure_esbuild_downloaded(&factory).await?;

  let resolver = factory.resolver().await?.clone();
  let module_load_preparer = factory.module_load_preparer().await?.clone();
  let root_permissions = factory.root_permissions_container()?;
  let npm_resolver = factory.npm_resolver().await?;
  let node_resolver = factory.node_resolver().await?;
  let cli_options = factory.cli_options()?;
  let module_loader = factory.resolver_factory()?.module_loader()?;
  let sys = factory.sys();
  let init_cwd = cli_options.initial_cwd().to_path_buf();
  let module_graph_container =
    factory.main_module_graph_container().await?.clone();

  let (on_end_tx, on_end_rx) = tokio::sync::mpsc::channel(10);
  #[allow(clippy::arc_with_non_send_sync)]
  let plugin_handler = Arc::new(DenoPluginHandler {
    file_fetcher: factory.file_fetcher()?.clone(),
    resolver: resolver.clone(),
    module_load_preparer,
    module_graph_container,
    permissions: root_permissions.clone(),
    module_loader: module_loader.clone(),
    externals_matcher: if bundle_flags.external.is_empty() {
      None
    } else {
      Some(ExternalsMatcher::new(&bundle_flags.external, &init_cwd))
    },
    on_end_tx,
    parsed_source_cache: factory.parsed_source_cache()?.clone(),
    cjs_tracker: factory.cjs_tracker()?.clone(),
    emitter: factory.emitter()?.clone(),
    deferred_resolve_errors: Default::default(),
  });

  let resolved_entrypoints =
    resolve_entrypoints(&resolver, &init_cwd, &bundle_flags.entrypoints)?;
  let _ = plugin_handler
    .prepare_module_load(&resolved_entrypoints)
    .await;

  let roots =
    resolve_roots(resolved_entrypoints, sys, npm_resolver, node_resolver);
  let _ = plugin_handler.prepare_module_load(&roots).await;

  let esbuild = EsbuildService::new(
    esbuild_path,
    esbuild::ESBUILD_VERSION,
    plugin_handler.clone(),
    Default::default(),
  )
  .await
  .unwrap();
  let client = esbuild.client().clone();

  tokio::spawn(async move {
    let res = esbuild.wait_for_exit().await;
    log::warn!("esbuild exited: {:?}", res);
  });

  let esbuild_flags = configure_esbuild_flags(bundle_flags);
  let entries = roots.into_iter().map(|e| ("".into(), e.into())).collect();
  let bundler = EsbuildBundler::new(
    client,
    plugin_handler.clone(),
    match bundle_flags.watch {
      true => BundlingMode::Watch,
      false => BundlingMode::OneShot,
    },
    on_end_rx,
    init_cwd.clone(),
    esbuild_flags,
    entries,
  );

  Ok(bundler)
}

pub async fn bundle(
  mut flags: Arc<Flags>,
  bundle_flags: BundleFlags,
) -> Result<(), AnyError> {
  {
    let flags_mut = Arc::make_mut(&mut flags);
    flags_mut.unstable_config.sloppy_imports = true;
  }
  let bundler = bundle_init(flags.clone(), &bundle_flags).await?;
  let init_cwd = bundler.cwd.clone();
  let start = std::time::Instant::now();
  let response = bundler.build().await?;
  let end = std::time::Instant::now();
  let duration = end.duration_since(start);

  if bundle_flags.watch {
    return bundle_watch(
      flags,
      bundler,
      bundle_flags.minify,
      bundle_flags.platform,
    )
    .await;
  }

  handle_esbuild_errors_and_warnings(
    &response,
    &init_cwd,
    &bundler.plugin_handler.take_deferred_resolve_errors(),
  );

  if response.errors.is_empty() {
    let metafile = metafile_from_response(&response)?;
    let output_infos = process_result(
      &response,
      &init_cwd,
      *DISABLE_HACK && matches!(bundle_flags.platform, BundlePlatform::Deno),
      bundle_flags.minify,
    )?;

    if bundle_flags.output_dir.is_some() || bundle_flags.output_path.is_some() {
      print_finished_message(&metafile, &output_infos, duration)?;
    }
  }

  if !response.errors.is_empty() {
    deno_core::anyhow::bail!("bundling failed");
  }

  Ok(())
}

fn metafile_from_response(
  response: &BuildResponse,
) -> Result<esbuild_client::Metafile, AnyError> {
  Ok(serde_json::from_str::<esbuild_client::Metafile>(
    response.metafile.as_deref().ok_or_else(|| {
      deno_core::anyhow::anyhow!("expected a metafile to be present")
    })?,
  )?)
}

async fn bundle_watch(
  flags: Arc<Flags>,
  bundler: EsbuildBundler,
  minified: bool,
  platform: BundlePlatform,
) -> Result<(), AnyError> {
  let initial_roots = bundler
    .roots
    .iter()
    .filter_map(|(_, root)| {
      let url = Url::parse(root).ok()?;
      deno_path_util::url_to_file_path(&url).ok()
    })
    .collect::<Vec<_>>();
  let current_roots = Rc::new(RefCell::new(initial_roots.clone()));
  let bundler = Rc::new(tokio::sync::Mutex::new(bundler));
  let mut print_config =
    crate::util::file_watcher::PrintConfig::new_with_banner(
      "Watcher", "Bundle", true,
    );
  print_config.print_finished = false;
  crate::util::file_watcher::watch_recv(
    flags,
    print_config,
    WatcherRestartMode::Automatic,
    move |_flags, watcher_communicator, changed_paths| {
      watcher_communicator.show_path_changed(changed_paths.clone());
      let bundler = Rc::clone(&bundler);
      let current_roots = current_roots.clone();
      Ok(async move {
        let mut bundler = bundler.lock().await;
        let start = std::time::Instant::now();
        if let Some(changed_paths) = changed_paths {
          bundler
            .plugin_handler
            .reload_specifiers(&changed_paths)
            .await?;
        }
        let response = bundler.rebuild().await?;
        handle_esbuild_errors_and_warnings(
          &response,
          &bundler.cwd,
          &bundler.plugin_handler.take_deferred_resolve_errors(),
        );
        if response.errors.is_empty() {
          let metafile = metafile_from_response(&response)?;
          let output_infos = process_result(
            &response,
            &bundler.cwd,
            *DISABLE_HACK && matches!(platform, BundlePlatform::Deno),
            minified,
          )?;
          print_finished_message(&metafile, &output_infos, start.elapsed())?;

          let new_watched = get_input_paths_for_watch(&response);
          *current_roots.borrow_mut() = new_watched.clone();
          let _ = watcher_communicator.watch_paths(new_watched);
        } else {
          let _ =
            watcher_communicator.watch_paths(current_roots.borrow().clone());
        }

        Ok(())
      })
    },
  )
  .boxed_local()
  .await?;

  Ok(())
}

fn get_input_paths_for_watch(response: &BuildResponse) -> Vec<PathBuf> {
  let metafile = serde_json::from_str::<esbuild_client::Metafile>(
    response
      .metafile
      .as_deref()
      .expect("metafile is required for watch mode"),
  )
  .unwrap();

  metafile
    .inputs
    .keys()
    .cloned()
    .map(PathBuf::from)
    .collect::<Vec<_>>()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BundlingMode {
  OneShot,
  Watch,
}

pub struct EsbuildBundler {
  client: esbuild_client::ProtocolClient,
  plugin_handler: Arc<DenoPluginHandler>,
  on_end_rx: tokio::sync::mpsc::Receiver<esbuild_client::OnEndArgs>,
  mode: BundlingMode,
  cwd: PathBuf,
  flags: EsbuildFlags,
  roots: Vec<(String, String)>,
}

impl EsbuildBundler {
  pub fn new(
    client: esbuild_client::ProtocolClient,
    plugin_handler: Arc<DenoPluginHandler>,
    mode: BundlingMode,
    on_end_rx: tokio::sync::mpsc::Receiver<esbuild_client::OnEndArgs>,
    cwd: PathBuf,
    flags: EsbuildFlags,
    roots: Vec<(String, String)>,
  ) -> EsbuildBundler {
    EsbuildBundler {
      client,
      plugin_handler,
      on_end_rx,
      mode,
      cwd,
      flags,
      roots,
    }
  }

  // When doing a watch build, we're actually enabling the
  // "context" mode of esbuild. That leaves esbuild running and
  // waits for a rebuild to be triggered. The initial build request
  // doesn't actually do anything, it's just registering the args/flags
  // we're going to use for all of the rebuilds.
  fn make_build_request(&self) -> protocol::BuildRequest {
    protocol::BuildRequest {
      entries: self.roots.clone(),
      key: 0,
      flags: self.flags.to_flags(),
      write: false,
      stdin_contents: None.into(),
      stdin_resolve_dir: None.into(),
      abs_working_dir: self.cwd.to_string_lossy().into_owned(),
      context: matches!(self.mode, BundlingMode::Watch),
      mangle_cache: None,
      node_paths: vec![],
      plugins: Some(vec![protocol::BuildPlugin {
        name: "deno".into(),
        on_start: false,
        on_end: matches!(self.mode, BundlingMode::Watch),
        on_resolve: (vec![protocol::OnResolveSetupOptions {
          id: 0,
          filter: ".*".into(),
          namespace: "".into(),
        }]),
        on_load: vec![protocol::OnLoadSetupOptions {
          id: 0,
          filter: ".*".into(),
          namespace: "".into(),
        }],
      }]),
    }
  }

  async fn build(&self) -> Result<BuildResponse, AnyError> {
    let response = self
      .client
      .send_build_request(self.make_build_request())
      .await
      .unwrap();

    Ok(response)
  }

  async fn rebuild(&mut self) -> Result<BuildResponse, AnyError> {
    match self.mode {
      BundlingMode::OneShot => {
        panic!("rebuild not supported for one-shot mode")
      }
      BundlingMode::Watch => {
        let _response = self.client.send_rebuild_request(0).await.unwrap();
        let response = self.on_end_rx.recv().await.unwrap();
        Ok(response.into())
      }
    }
  }
}

// TODO(nathanwhit): MASSIVE HACK
// See tests::specs::bundle::requires_node_builtin for why this is needed.
// Without this hack, that test would fail with "Dynamic require of "util" is not supported"
fn replace_require_shim(contents: &str, minified: bool) -> String {
  if minified {
    let re = lazy_regex::regex!(
      r#"var (\w+)\s*=\((\w+)\s*=>typeof require<"u"\?require:typeof Proxy<"u"\?new Proxy\((\w+)\,\{get:\(\w+,\w+\)=>\(typeof require<"u"\?require:\w+\)\[l\]\}\):(\w+)\)\(function\(\w+\)\{if\(typeof require<"u"\)return require\.apply\(this\,arguments\);throw Error\('Dynamic require of "'\+\w+\+'" is not supported'\)\}\);"#
    );
    re.replace(contents, |c: &regex::Captures<'_>| {
      let var_name = c.get(1).unwrap().as_str();
      format!("import{{createRequire}} from \"node:module\";var {var_name}=createRequire(import.meta.url);")
    }).into_owned()
  } else {
    let re = lazy_regex::regex!(
      r#"var __require = (/\* @__PURE__ \*/)?\s*\(\(\w+\) => typeof require !== "undefined" \? require : typeof Proxy !== "undefined" \? new Proxy\(\w+, \{\s*  get: \(\w+, \w+\) => \(typeof require !== "undefined" \? require : \w+\)\[\w+\]\s*\}\) : \w+\)\(function\(\w+\) \{\s*  if \(typeof require !== "undefined"\) return require\.apply\(this, arguments\);\s*  throw Error\('Dynamic require of "' \+ \w+ \+ '" is not supported'\);\s*\}\);"#
    );
    re.replace_all(
      contents,
      r#"import { createRequire } from "node:module";
var __require = createRequire(import.meta.url);
"#,
    )
    .into_owned()
  }
}

fn format_location(
  location: &esbuild_client::protocol::Location,
  current_dir: &Path,
) -> String {
  let url =
    deno_path_util::resolve_url_or_path(location.file.as_str(), current_dir)
      .map(|url| deno_terminal::colors::cyan(url.into()))
      .unwrap_or(deno_terminal::colors::cyan(location.file.clone()));

  format!(
    "{}:{}:{}",
    url,
    deno_terminal::colors::yellow(location.line),
    deno_terminal::colors::yellow(location.column)
  )
}

fn format_note(
  note: &esbuild_client::protocol::Note,
  current_dir: &Path,
) -> String {
  format!(
    "{}: {}{}",
    deno_terminal::colors::magenta("note"),
    note.text,
    if let Some(location) = &note.location {
      format!("\n    {}", format_location(location, current_dir))
    } else {
      String::new()
    }
  )
}

// not very efficient, but it's only for error messages
fn add_indent(s: &str, indent: &str) -> String {
  let lines = s
    .lines()
    .map(|line| format!("{}{}", indent, line))
    .collect::<Vec<_>>();
  lines.join("\n")
}

fn format_message(
  message: &esbuild_client::protocol::Message,
  current_dir: &Path,
) -> String {
  format!(
    "{}{}{}{}",
    message.text,
    if message.id.is_empty() {
      String::new()
    } else {
      format!("[{}] ", message.id)
    },
    if let Some(location) = &message.location {
      if !message.text.contains(" at ") {
        format!("\n    at {}", format_location(location, current_dir))
      } else {
        String::new()
      }
    } else {
      String::new()
    },
    if !message.notes.is_empty() {
      let mut s = String::new();
      for note in &message.notes {
        s.push('\n');
        s.push_str(&add_indent(&format_note(note, current_dir), "    "));
      }
      s
    } else {
      String::new()
    }
  )
}
#[derive(Debug, thiserror::Error, JsError)]
#[class(generic)]
enum BundleError {
  #[error(transparent)]
  Resolver(#[from] deno_resolver::graph::ResolveWithGraphError),
  #[error(transparent)]
  Url(#[from] deno_core::url::ParseError),
  #[error(transparent)]
  ResolveNpmPkg(#[from] ResolvePkgFolderFromDenoModuleError),
  #[error(transparent)]
  SubpathResolve(#[from] PackageSubpathResolveError),
  #[error(transparent)]
  PathToUrlError(#[from] deno_path_util::PathToUrlError),
  #[error(transparent)]
  UrlToPathError(#[from] deno_path_util::UrlToFilePathError),
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error(transparent)]
  ResolveUrlOrPathError(#[from] deno_path_util::ResolveUrlOrPathError),
  #[error(transparent)]
  PrepareModuleLoad(#[from] crate::module_loader::PrepareModuleLoadError),
  #[error(transparent)]
  ResolveReqWithSubPath(#[from] deno_resolver::npm::ResolveReqWithSubPathError),
  #[error(transparent)]
  PackageReqReferenceParse(
    #[from] deno_semver::package::PackageReqReferenceParseError,
  ),
  #[allow(dead_code)]
  #[error("Http cache error")]
  HttpCache,
}

fn requested_type_from_map(
  map: &IndexMap<String, String>,
) -> RequestedModuleType<'_> {
  let type_ = map.get("type").map(|s| s.as_str());
  match type_ {
    Some("json") => RequestedModuleType::Json,
    Some("bytes") => RequestedModuleType::Bytes,
    Some("text") => RequestedModuleType::Text,
    Some(other) => RequestedModuleType::Other(other),
    None => RequestedModuleType::None,
  }
}

pub struct DeferredResolveError {
  path: String,
  error: ResolveWithGraphError,
}

pub struct DenoPluginHandler {
  file_fetcher: Arc<CliFileFetcher>,
  resolver: Arc<CliResolver>,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  module_graph_container: Arc<MainModuleGraphContainer>,
  permissions: PermissionsContainer,
  module_loader: Arc<CliDenoResolverModuleLoader>,
  externals_matcher: Option<ExternalsMatcher>,
  on_end_tx: tokio::sync::mpsc::Sender<esbuild_client::OnEndArgs>,
  deferred_resolve_errors: Arc<Mutex<Vec<DeferredResolveError>>>,

  parsed_source_cache: Arc<ParsedSourceCache>,
  cjs_tracker: Arc<CliCjsTracker>,
  emitter: Arc<CliEmitter>,
}

impl DenoPluginHandler {
  fn take_deferred_resolve_errors(&self) -> Vec<DeferredResolveError> {
    std::mem::take(&mut *self.deferred_resolve_errors.lock())
  }
}

/*#[derive(Debug, Clone)]
pub struct OnResolveArgs {
    pub path: String,
    pub importer: Option<String>,
    pub kind: ImportKind,
    pub namespace: Option<String>,
    pub resolve_dir: Option<String>,
    pub with: IndexMap<String, String>,
} */
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum PluginImportKind {
  EntryPoint,
  ImportStatement,
  RequireCall,
  DynamicImport,
  RequireResolve,
  ImportRule,
  ComposesFrom,
  UrlToken,
}

impl From<protocol::ImportKind> for PluginImportKind {
  fn from(kind: protocol::ImportKind) -> Self {
    match kind {
      protocol::ImportKind::EntryPoint => PluginImportKind::EntryPoint,
      protocol::ImportKind::ImportStatement => {
        PluginImportKind::ImportStatement
      }
      protocol::ImportKind::RequireCall => PluginImportKind::RequireCall,
      protocol::ImportKind::DynamicImport => PluginImportKind::DynamicImport,
      protocol::ImportKind::RequireResolve => PluginImportKind::RequireResolve,
      protocol::ImportKind::ImportRule => PluginImportKind::ImportRule,
      protocol::ImportKind::ComposesFrom => PluginImportKind::ComposesFrom,
      protocol::ImportKind::UrlToken => PluginImportKind::UrlToken,
    }
  }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginOnResolveArgs {
  path: String,
  importer: Option<String>,
  kind: PluginImportKind,
  namespace: Option<String>,
  resolve_dir: Option<String>,
  with: IndexMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginOnLoadArgs {
  path: String,
  namespace: String,
  suffix: String,
  with: IndexMap<String, String>,
}

#[async_trait::async_trait(?Send)]
impl esbuild_client::PluginHandler for DenoPluginHandler {
  async fn on_resolve(
    &self,
    args: esbuild_client::OnResolveArgs,
  ) -> Result<Option<esbuild_client::OnResolveResult>, AnyError> {
    log::debug!("{}: {args:?}", deno_terminal::colors::cyan("on_resolve"));
    if let Some(matcher) = &self.externals_matcher
      && matcher.is_pre_resolve_match(&args.path)
    {
      return Ok(Some(esbuild_client::OnResolveResult {
        external: Some(true),
        path: Some(args.path),
        plugin_name: Some("deno".to_string()),
        plugin_data: None,
        ..Default::default()
      }));
    }

    let result = self.bundle_resolve(
      &args.path,
      args.importer.as_deref(),
      args.resolve_dir.as_deref(),
      args.kind,
      args.with,
    );

    let result = match result {
      Ok(r) => r,
      Err(e) => {
        return Ok(Some(esbuild_client::OnResolveResult {
          errors: Some(vec![esbuild_client::protocol::PartialMessage {
            id: "deno_error".into(),
            plugin_name: "deno".into(),
            text: e.to_string(),
            ..Default::default()
          }]),
          ..Default::default()
        }));
      }
    };

    Ok(result.map(|r| {
      // TODO(nathanwhit): remap the resolved path to be relative
      // to the output file. It will be tricky to figure out which
      // output file this import will end up in. We may have to use the metafile and rewrite at the end
      let is_external = r.starts_with("node:")
        || self
          .externals_matcher
          .as_ref()
          .map(|matcher| matcher.is_post_resolve_match(&r))
          .unwrap_or(false);

      esbuild_client::OnResolveResult {
        namespace: if r.starts_with("jsr:")
          || r.starts_with("https:")
          || r.starts_with("http:")
          || r.starts_with("data:")
        {
          Some("deno".into())
        } else {
          None
        },
        external: Some(is_external),
        path: Some(r),
        plugin_name: Some("deno".to_string()),
        plugin_data: None,
        ..Default::default()
      }
    }))
  }

  async fn on_load(
    &self,
    args: esbuild_client::OnLoadArgs,
  ) -> Result<Option<esbuild_client::OnLoadResult>, AnyError> {
    log::debug!("{}: {args:?}", deno_terminal::colors::cyan("on_load"));
    let result = self
      .bundle_load(&args.path, &requested_type_from_map(&args.with))
      .await;
    let result = match result {
      Ok(r) => r,
      Err(e) => {
        if e.is_unsupported_media_type() {
          return Ok(None);
        }
        return Ok(Some(esbuild_client::OnLoadResult {
          errors: Some(vec![esbuild_client::protocol::PartialMessage {
            plugin_name: "deno".into(),
            text: e.to_string(),
            ..Default::default()
          }]),
          plugin_name: Some("deno".to_string()),
          ..Default::default()
        }));
      }
    };
    log::trace!(
      "{}: {:?}",
      deno_terminal::colors::magenta("on_load"),
      result.as_ref().map(|(code, loader)| format!(
        "{}: {:?}",
        String::from_utf8_lossy(code),
        loader
      ))
    );
    if let Some((code, loader)) = result {
      Ok(Some(esbuild_client::OnLoadResult {
        contents: Some(code),
        loader: Some(loader),
        ..Default::default()
      }))
    } else {
      Ok(None)
    }
  }

  async fn on_start(
    &self,
    _args: esbuild_client::OnStartArgs,
  ) -> Result<Option<esbuild_client::OnStartResult>, AnyError> {
    Ok(None)
  }

  async fn on_end(
    &self,
    _args: esbuild_client::OnEndArgs,
  ) -> Result<Option<esbuild_client::OnEndResult>, AnyError> {
    self.on_end_tx.send(_args).await?;
    Ok(None)
  }
}

fn import_kind_to_resolution_mode(
  kind: esbuild_client::protocol::ImportKind,
) -> ResolutionMode {
  match kind {
    protocol::ImportKind::EntryPoint
    | protocol::ImportKind::ImportStatement
    | protocol::ImportKind::ComposesFrom
    | protocol::ImportKind::DynamicImport
    | protocol::ImportKind::ImportRule
    | protocol::ImportKind::UrlToken => ResolutionMode::Import,
    protocol::ImportKind::RequireCall
    | protocol::ImportKind::RequireResolve => ResolutionMode::Require,
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum BundleLoadError {
  #[class(inherit)]
  #[error(transparent)]
  Fetch(#[from] deno_resolver::file_fetcher::FetchError),
  #[class(inherit)]
  #[error(transparent)]
  LoadCodeSource(#[from] LoadCodeSourceError),
  #[class(inherit)]
  #[error(transparent)]
  ResolveUrlOrPath(#[from] deno_path_util::ResolveUrlOrPathError),
  #[class(inherit)]
  #[error(transparent)]
  ResolveWithGraph(#[from] ResolveWithGraphError),
  #[class(generic)]
  #[error("Wasm modules are not implemented in deno bundle.")]
  WasmUnsupported,
  #[class(generic)]
  #[error("UTF-8 conversion error")]
  Utf8(#[from] std::str::Utf8Error),
  #[class(generic)]
  #[error("UTF-8 conversion error")]
  StringUtf8(#[from] std::string::FromUtf8Error),
  #[class(generic)]
  #[error("Parse error")]
  Parse(#[from] deno_ast::ParseDiagnostic),
  #[class(generic)]
  #[error("Emit error")]
  Emit(#[from] deno_ast::EmitError),
  #[class(generic)]
  #[error("Prepare module load error")]
  PrepareModuleLoad(#[from] crate::module_loader::PrepareModuleLoadError),

  #[class(generic)]
  #[error("Package.json load error")]
  PackageJsonLoadError(#[from] node_resolver::errors::PackageJsonLoadError),

  #[class(generic)]
  #[error("Emit parsed source helper error")]
  EmitParsedSourceHelperError(
    #[from] deno_resolver::emit::EmitParsedSourceHelperError,
  ),
}

impl BundleLoadError {
  pub fn is_unsupported_media_type(&self) -> bool {
    match self {
      BundleLoadError::LoadCodeSource(e) => match e.as_kind() {
        LoadCodeSourceErrorKind::LoadPreparedModule(e) => match e.as_kind() {
          LoadPreparedModuleErrorKind::Graph(e) => matches!(
            e.error.as_kind(),
            ModuleErrorKind::UnsupportedMediaType { .. },
          ),
          _ => false,
        },
        _ => false,
      },
      _ => false,
    }
  }
}

fn maybe_ignorable_resolution_error(
  error: &ResolveWithGraphError,
) -> Option<String> {
  if let deno_resolver::graph::ResolveWithGraphErrorKind::Resolve(e) =
    error.as_kind()
    && let deno_resolver::DenoResolveErrorKind::Node(node_err) = e.as_kind()
    && let node_resolver::errors::NodeResolveErrorKind::PackageResolve(pkg_err) =
      node_err.as_kind()
    && let node_resolver::errors::PackageResolveErrorKind::PackageFolderResolve(
      pkg_folder_err,
    ) = pkg_err.as_kind()
    && let node_resolver::errors::PackageFolderResolveErrorKind::PackageNotFound(
      PackageNotFoundError { package_name, .. },
    ) = pkg_folder_err.as_kind()
  {
    Some(package_name.to_string())
  } else if let deno_resolver::graph::ResolveWithGraphErrorKind::Resolution(
    deno_graph::ResolutionError::ResolverError {
      error: resolve_error,
      specifier,
      ..
    },
  ) = error.as_kind()
    && let deno_graph::source::ResolveError::ImportMap(import_map_err) =
      resolve_error.deref()
    && let import_map::ImportMapErrorKind::UnmappedBareSpecifier(..) =
      import_map_err.as_kind()
  {
    Some(specifier.to_string())
  } else {
    None
  }
}

impl DenoPluginHandler {
  async fn reload_specifiers(
    &self,
    specifiers: &[PathBuf],
  ) -> Result<(), AnyError> {
    let mut graph_permit =
      self.module_graph_container.acquire_update_permit().await;
    let graph = graph_permit.graph_mut();
    let mut specifiers_vec = Vec::with_capacity(specifiers.len());
    for specifier in specifiers {
      let specifier = deno_path_util::url_from_file_path(specifier)?;
      specifiers_vec.push(specifier);
    }
    self
      .module_load_preparer
      .reload_specifiers(graph, specifiers_vec, false, self.permissions.clone())
      .await?;
    graph_permit.commit();
    Ok(())
  }

  #[allow(clippy::result_large_err)]
  fn bundle_resolve(
    &self,
    path: &str,
    importer: Option<&str>,
    resolve_dir: Option<&str>,
    kind: esbuild_client::protocol::ImportKind,
    with: IndexMap<String, String>,
  ) -> Result<Option<String>, BundleError> {
    log::debug!(
      "bundle_resolve: {:?} {:?} {:?} {:?} {:?}",
      path,
      importer,
      resolve_dir,
      kind,
      with
    );
    let mut resolve_dir = resolve_dir.unwrap_or("").to_string();
    let resolver = self.resolver.clone();
    if !resolve_dir.ends_with(std::path::MAIN_SEPARATOR) {
      resolve_dir.push(std::path::MAIN_SEPARATOR);
    }
    let resolve_dir_path = Path::new(&resolve_dir);
    let mut referrer =
      resolve_url_or_path(importer.unwrap_or(""), resolve_dir_path)
        .unwrap_or_else(|_| {
          Url::from_directory_path(std::env::current_dir().unwrap()).unwrap()
        });
    if referrer.scheme() == "file" {
      let pth = referrer.to_file_path().unwrap();
      if (pth.is_dir()) && !pth.ends_with(std::path::MAIN_SEPARATOR_STR) {
        referrer.set_path(&format!(
          "{}{}",
          referrer.path(),
          std::path::MAIN_SEPARATOR
        ));
      }
    }

    log::debug!(
      "{}: {} {} {} {:?}",
      deno_terminal::colors::magenta("op_bundle_resolve"),
      path,
      resolve_dir,
      referrer,
      import_kind_to_resolution_mode(kind)
    );

    let graph = self.module_graph_container.graph();
    let result = resolver.resolve_with_graph(
      &graph,
      path,
      &referrer,
      Position::new(0, 0),
      ResolveWithGraphOptions {
        mode: import_kind_to_resolution_mode(kind),
        kind: NodeResolutionKind::Execution,
        maintain_npm_specifiers: false,
      },
    );

    log::debug!(
      "{}: {:?}",
      deno_terminal::colors::cyan("op_bundle_resolve result"),
      result
    );

    match result {
      Ok(specifier) => Ok(Some(file_path_or_url(specifier)?)),
      Err(e) => {
        log::debug!("{}: {:?}", deno_terminal::colors::red("error"), e);
        if let Some(specifier) = maybe_ignorable_resolution_error(&e) {
          log::debug!(
            "{}: resolution failed, but maybe ignorable",
            deno_terminal::colors::red("warn")
          );
          self
            .deferred_resolve_errors
            .lock()
            .push(DeferredResolveError {
              path: specifier,
              error: e,
            });
          // we return None here because this lets esbuild choose to ignore the failure
          // for fallible imports/requires
          return Ok(None);
        }
        Err(BundleError::Resolver(e))
      }
    }
  }

  async fn prepare_module_load(
    &self,
    specifiers: &[ModuleSpecifier],
  ) -> Result<(), BundleLoadError> {
    let mut graph_permit =
      self.module_graph_container.acquire_update_permit().await;
    let graph: &mut deno_graph::ModuleGraph = graph_permit.graph_mut();
    self
      .module_load_preparer
      .prepare_module_load(
        graph,
        specifiers,
        PrepareModuleLoadOptions {
          is_dynamic: false,
          lib: TsTypeLib::default(),
          permissions: self.permissions.clone(),
          ext_overwrite: None,
          allow_unknown_media_types: true,
          skip_graph_roots_validation: true,
        },
      )
      .await?;
    graph_permit.commit();
    Ok(())
  }

  async fn bundle_load(
    &self,
    specifier: &str,
    requested_type: &RequestedModuleType<'_>,
  ) -> Result<Option<(Vec<u8>, esbuild_client::BuiltinLoader)>, BundleLoadError>
  {
    log::debug!(
      "{}: {:?} {:?}",
      deno_terminal::colors::magenta("bundle_load"),
      specifier,
      requested_type
    );

    let specifier = deno_path_util::resolve_url_or_path(
      specifier,
      Path::new(""), // should be absolute already, feels kind of hacky though
    )?;
    let (specifier, media_type) =
      if let RequestedModuleType::Bytes = requested_type {
        (specifier, MediaType::Unknown)
      } else if let RequestedModuleType::Text = requested_type {
        (specifier, MediaType::Unknown)
      } else if let Some((specifier, media_type, _)) =
        self.specifier_and_type_from_graph(&specifier)?
      {
        (specifier, media_type)
      } else {
        log::debug!(
          "{}: no specifier and type from graph for {}",
          deno_terminal::colors::yellow("warn"),
          specifier
        );

        if specifier.scheme() == "data" {
          return Ok(Some((
            specifier.to_string().as_bytes().to_vec(),
            esbuild_client::BuiltinLoader::DataUrl,
          )));
        }

        let (media_type, _) =
          deno_media_type::resolve_media_type_and_charset_from_content_type(
            &specifier, None,
          );
        if media_type == deno_media_type::MediaType::Unknown {
          return Ok(None);
        }
        (specifier, media_type)
      };

    let graph = self.module_graph_container.graph();
    let module_or_asset = self
      .module_loader
      .load(&graph, &specifier, None, requested_type)
      .await;
    let module_or_asset = match module_or_asset {
      Ok(module_or_asset) => module_or_asset,
      Err(e) => match e.as_kind() {
        LoadCodeSourceErrorKind::LoadUnpreparedModule(_) => {
          let file = self
            .file_fetcher
            .fetch_bypass_permissions(&specifier)
            .await?;
          let media_type = MediaType::from_specifier_and_headers(
            &specifier,
            file.maybe_headers.as_ref(),
          );
          match requested_type {
            RequestedModuleType::Text | RequestedModuleType::Bytes => {
              return self
                .create_module_response(
                  &graph,
                  &specifier,
                  media_type,
                  &file.source,
                  Some(requested_type),
                )
                .await
                .map(Some);
            }
            RequestedModuleType::None
            | RequestedModuleType::Json
            | RequestedModuleType::Other(_) => {
              if media_type.is_emittable() {
                let str = String::from_utf8_lossy(&file.source);
                let value = str.into();
                let source = self
                  .maybe_transpile(&file.url, media_type, &value, None)
                  .await?;
                return self
                  .create_module_response(
                    &graph,
                    &file.url,
                    media_type,
                    source.as_bytes(),
                    Some(requested_type),
                  )
                  .await
                  .map(Some);
              } else {
                return self
                  .create_module_response(
                    &graph,
                    &file.url,
                    media_type,
                    &file.source,
                    Some(requested_type),
                  )
                  .await
                  .map(Some);
              }
            }
          }
        }
        _ => return Err(e.into()),
      },
    };
    let loaded_code = match module_or_asset {
      LoadedModuleOrAsset::Module(loaded_module) => loaded_module.source,
      LoadedModuleOrAsset::ExternalAsset {
        specifier,
        statically_analyzable: _,
      } => LoadedModuleSource::ArcBytes(
        self
          .file_fetcher
          .fetch(&specifier, &self.permissions)
          .await?
          .source,
      ),
    };

    Ok(Some(
      self
        .create_module_response(
          &graph,
          &specifier,
          media_type,
          loaded_code.as_bytes(),
          Some(requested_type),
        )
        .await?,
    ))
  }

  async fn create_module_response(
    &self,
    graph: &deno_graph::ModuleGraph,
    specifier: &Url,
    media_type: MediaType,
    source: &[u8],
    requested_type: Option<&RequestedModuleType<'_>>,
  ) -> Result<(Vec<u8>, esbuild_client::BuiltinLoader), BundleLoadError> {
    match requested_type {
      Some(RequestedModuleType::Text) => {
        return Ok((source.to_vec(), esbuild_client::BuiltinLoader::Text));
      }
      Some(RequestedModuleType::Bytes) => {
        return Ok((source.to_vec(), esbuild_client::BuiltinLoader::Binary));
      }
      Some(RequestedModuleType::Json) => {
        return Ok((source.to_vec(), esbuild_client::BuiltinLoader::Json));
      }
      Some(RequestedModuleType::Other(_) | RequestedModuleType::None)
      | None => {}
    }
    if matches!(
      media_type,
      MediaType::JavaScript
        | MediaType::TypeScript
        | MediaType::Mjs
        | MediaType::Mts
        | MediaType::Cjs
        | MediaType::Cts
        | MediaType::Jsx
        | MediaType::Tsx
    ) && !graph.roots.contains(specifier)
    {
      let module_graph_container = self.module_graph_container.clone();
      let specifier = specifier.clone();
      let code = source.to_vec();
      let code = tokio::task::spawn_blocking(move || {
        Self::apply_transform(
          &module_graph_container,
          &specifier,
          media_type,
          &String::from_utf8(code)?,
        )
      })
      .await
      .unwrap()?;
      Ok((code.into_bytes(), media_type_to_loader(media_type)))
    } else {
      Ok((source.to_vec(), media_type_to_loader(media_type)))
    }
  }

  async fn maybe_transpile(
    &self,
    specifier: &Url,
    media_type: MediaType,
    source: &Arc<str>,
    is_known_script: Option<bool>,
  ) -> Result<Arc<str>, BundleLoadError> {
    let parsed_source = self.parsed_source_cache.get_matching_parsed_source(
      specifier,
      media_type,
      source.clone(),
    )?;
    let is_cjs = if let Some(is_known_script) = is_known_script {
      self.cjs_tracker.is_cjs_with_known_is_script(
        specifier,
        media_type,
        is_known_script,
      )?
    } else {
      self.cjs_tracker.is_maybe_cjs(specifier, media_type)?
        && parsed_source.compute_is_script()
    };
    let module_kind = ModuleKind::from_is_cjs(is_cjs);
    let source = self
      .emitter
      .maybe_emit_parsed_source(parsed_source, module_kind)
      .await?;
    Ok(source)
  }

  #[allow(clippy::result_large_err)]
  fn apply_transform(
    module_graph_container: &MainModuleGraphContainer,
    specifier: &ModuleSpecifier,
    media_type: deno_ast::MediaType,
    code: &str,
  ) -> Result<String, BundleLoadError> {
    let mut transform = transform::BundleImportMetaMainTransform::new(
      module_graph_container.graph().roots.contains(specifier),
    );
    let parsed_source = deno_ast::parse_program_with_post_process(
      deno_ast::ParseParams {
        specifier: specifier.clone(),
        text: code.into(),
        media_type,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
      },
      |mut program, _| {
        use deno_ast::swc::ecma_visit::VisitMut;
        transform.visit_mut_program(&mut program);
        program
      },
    )?;
    let code = deno_ast::emit(
      parsed_source.program_ref(),
      &parsed_source.comments().as_single_threaded(),
      &deno_ast::SourceMap::default(),
      &EmitOptions {
        source_map: deno_ast::SourceMapOption::None,
        ..Default::default()
      },
    )?;
    Ok(code.text)
  }

  #[allow(clippy::result_large_err)]
  fn specifier_and_type_from_graph(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<
    Option<(
      ModuleSpecifier,
      deno_ast::MediaType,
      esbuild_client::BuiltinLoader,
    )>,
    BundleLoadError,
  > {
    let graph = self.module_graph_container.graph();
    let Some(module) = graph.get(specifier) else {
      return Ok(None);
    };
    let (specifier, media_type, loader) = match module {
      deno_graph::Module::Js(js_module) => (
        js_module.specifier.clone(),
        js_module.media_type,
        media_type_to_loader(js_module.media_type),
      ),
      deno_graph::Module::Json(json_module) => (
        json_module.specifier.clone(),
        deno_ast::MediaType::Json,
        esbuild_client::BuiltinLoader::Json,
      ),
      deno_graph::Module::Wasm(_) => {
        return Err(BundleLoadError::WasmUnsupported);
      }
      deno_graph::Module::Npm(module) => {
        let url = self.resolver.resolve_npm_nv_ref(
          &module.nv_reference,
          None,
          ResolutionMode::Import,
          NodeResolutionKind::Execution,
        )?;
        let (media_type, _charset) =
          deno_media_type::resolve_media_type_and_charset_from_content_type(
            &url, None,
          );
        (url, media_type, media_type_to_loader(media_type))
      }
      deno_graph::Module::Node(_) => {
        return Ok(None);
      }
      deno_graph::Module::External(_) => {
        return Ok(None);
      }
    };
    Ok(Some((specifier, media_type, loader)))
  }
}

fn file_path_or_url(
  url: Url,
) -> Result<String, deno_path_util::UrlToFilePathError> {
  if url.scheme() == "file" {
    Ok(
      deno_path_util::url_to_file_path(&url)?
        .to_string_lossy()
        .into(),
    )
  } else {
    Ok(url.into())
  }
}

fn media_type_to_loader(
  media_type: deno_media_type::MediaType,
) -> esbuild_client::BuiltinLoader {
  use deno_ast::MediaType::*;
  match media_type {
    JavaScript | Cjs | Mjs | Mts => esbuild_client::BuiltinLoader::Js,
    TypeScript | Cts | Dts | Dmts | Dcts => esbuild_client::BuiltinLoader::Ts,
    Jsx | Tsx => esbuild_client::BuiltinLoader::Jsx,
    Css => esbuild_client::BuiltinLoader::Css,
    Json => esbuild_client::BuiltinLoader::Json,
    SourceMap => esbuild_client::BuiltinLoader::Text,
    Html => esbuild_client::BuiltinLoader::Text,
    Sql => esbuild_client::BuiltinLoader::Text,
    Wasm => esbuild_client::BuiltinLoader::Binary,
    Unknown => esbuild_client::BuiltinLoader::Binary,
    // _ => esbuild_client::BuiltinLoader::External,
  }
}

fn resolve_url_or_path_absolute(
  specifier: &str,
  current_dir: &Path,
) -> Result<Url, AnyError> {
  if deno_path_util::specifier_has_uri_scheme(specifier) {
    Ok(Url::parse(specifier)?)
  } else {
    let path = current_dir.join(specifier);
    let path = deno_path_util::normalize_path(Cow::Owned(path));
    let path = path.canonicalize()?;
    Ok(deno_path_util::url_from_file_path(&path)?)
  }
}

fn resolve_entrypoints(
  resolver: &CliResolver,
  init_cwd: &Path,
  entrypoints: &[String],
) -> Result<Vec<Url>, AnyError> {
  let entrypoints = entrypoints
    .iter()
    .map(|e| resolve_url_or_path_absolute(e, init_cwd))
    .collect::<Result<Vec<_>, _>>()?;

  let init_cwd_url = Url::from_directory_path(init_cwd).unwrap();

  let mut resolved = Vec::with_capacity(entrypoints.len());

  for e in &entrypoints {
    let r = resolver.resolve(
      e.as_str(),
      &init_cwd_url,
      Position::new(0, 0),
      ResolutionMode::Import,
      NodeResolutionKind::Execution,
    )?;
    resolved.push(r);
  }
  Ok(resolved)
}

fn resolve_roots(
  entrypoints: Vec<Url>,
  sys: CliSys,
  npm_resolver: &CliNpmResolver,
  node_resolver: &CliNodeResolver,
) -> Vec<Url> {
  let mut roots = Vec::with_capacity(entrypoints.len());

  for url in entrypoints {
    let root = match NpmPackageReqReference::from_specifier(&url) {
      Ok(v) => {
        let referrer =
          ModuleSpecifier::from_directory_path(sys.env_current_dir().unwrap())
            .unwrap();
        let package_folder = npm_resolver
          .resolve_pkg_folder_from_deno_module_req(v.req(), &referrer)
          .unwrap();
        let Ok(main_module) =
          node_resolver.resolve_binary_export(&package_folder, v.sub_path())
        else {
          roots.push(url);
          continue;
        };
        Url::from_file_path(&main_module).unwrap()
      }
      _ => url,
    };
    roots.push(root)
  }

  roots
}

/// Ensure that an Esbuild binary for the current os/arch is downloaded
/// and ready to use and then return path to it.
async fn ensure_esbuild_downloaded(
  factory: &CliFactory,
) -> Result<PathBuf, AnyError> {
  let installer_factory = factory.npm_installer_factory()?;
  let deno_dir = factory.deno_dir()?;
  let npmrc = factory.npmrc()?;
  let npm_registry_info = installer_factory.registry_info_provider()?;
  let resolver_factory = factory.resolver_factory()?;
  let workspace_factory = resolver_factory.workspace_factory();

  let esbuild_path = esbuild::ensure_esbuild(
    deno_dir,
    npmrc,
    npm_registry_info,
    workspace_factory.workspace_npm_link_packages()?,
    installer_factory.tarball_cache()?,
    factory.npm_cache()?,
  )
  .await?;
  Ok(esbuild_path)
}

fn configure_esbuild_flags(bundle_flags: &BundleFlags) -> EsbuildFlags {
  let mut builder = EsbuildFlagsBuilder::default();

  builder
    .bundle(bundle_flags.inline_imports)
    .minify(bundle_flags.minify)
    .splitting(bundle_flags.code_splitting)
    .external(bundle_flags.external.clone())
    .tree_shaking(true)
    .format(match bundle_flags.format {
      BundleFormat::Esm => esbuild_client::Format::Esm,
      BundleFormat::Cjs => esbuild_client::Format::Cjs,
      BundleFormat::Iife => esbuild_client::Format::Iife,
    })
    .packages(match bundle_flags.packages {
      PackageHandling::External => esbuild_client::PackagesHandling::External,
      PackageHandling::Bundle => esbuild_client::PackagesHandling::Bundle,
    });

  if let Some(sourcemap_type) = bundle_flags.sourcemap {
    builder.sourcemap(match sourcemap_type {
      SourceMapType::Linked => esbuild_client::Sourcemap::Linked,
      SourceMapType::Inline => esbuild_client::Sourcemap::Inline,
      SourceMapType::External => esbuild_client::Sourcemap::External,
    });
  }

  if let Some(outdir) = bundle_flags.output_dir.clone() {
    builder.outdir(outdir);
  } else if let Some(output_path) = bundle_flags.output_path.clone() {
    builder.outfile(output_path);
  }
  builder.metafile(true);

  match bundle_flags.platform {
    deno_bundle_runtime::BundlePlatform::Browser => {
      builder.platform(esbuild_client::Platform::Browser);
    }
    deno_bundle_runtime::BundlePlatform::Deno => {}
  }

  builder.build().unwrap()
}

// extract the path from a message like "Could not resolve "path/to/file.ts""
fn esbuild_resolve_error_path(
  error: &esbuild_client::protocol::Message,
) -> Option<String> {
  let re = lazy_regex::regex!(r#"^Could not resolve "([^"]+)"#);
  re.captures(error.text.as_str())
    .map(|captures| captures.get(1).unwrap().as_str().to_string())
}

fn handle_esbuild_errors_and_warnings(
  response: &BuildResponse,
  init_cwd: &Path,
  deferred_resolve_errors: &[DeferredResolveError],
) {
  for error in &response.errors {
    if let Some(path) = esbuild_resolve_error_path(error)
      && let Some(deferred_resolve_error) =
        deferred_resolve_errors.iter().find(|e| e.path == path)
    {
      let error = protocol::Message {
        // use our own error message, as it has more detail
        text: deferred_resolve_error.error.to_string(),
        ..error.clone()
      };
      log::error!(
        "{}: {}",
        deno_terminal::colors::red_bold("error"),
        format_message(&error, init_cwd)
      );
      continue;
    }
    log::error!(
      "{}: {}",
      deno_terminal::colors::red_bold("error"),
      format_message(error, init_cwd)
    );
  }

  for warning in &response.warnings {
    log::warn!(
      "{}: {}",
      deno_terminal::colors::yellow("bundler warning"),
      format_message(warning, init_cwd)
    );
  }
}

fn is_js(path: &Path) -> bool {
  if let Some(ext) = path.extension() {
    matches!(
      ext.to_string_lossy().as_ref(),
      "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" | "mts" | "cts" | "dts"
    )
  } else {
    false
  }
}

pub struct OutputFileInfo {
  relative_path: PathBuf,
  size: usize,
  is_js: bool,
}
pub fn process_result(
  response: &BuildResponse,
  cwd: &Path,
  should_replace_require_shim: bool,
  minified: bool,
) -> Result<Vec<OutputFileInfo>, AnyError> {
  let mut exists_cache = std::collections::HashSet::new();
  let output_files = response
    .output_files
    .as_ref()
    .map(Cow::Borrowed)
    .unwrap_or_default();
  let mut output_infos = Vec::new();
  for file in output_files.iter() {
    let path = Path::new(&file.path);
    let relative_path =
      pathdiff::diff_paths(path, cwd).unwrap_or_else(|| path.to_path_buf());
    let is_js = is_js(path);
    let bytes = if is_js || file.path.ends_with("<stdout>") {
      let string = String::from_utf8(file.contents.clone())?;
      let string = if should_replace_require_shim {
        replace_require_shim(&string, minified)
      } else {
        string
      };
      Cow::Owned(string.into_bytes())
    } else {
      Cow::Borrowed(&file.contents)
    };

    if file.path.ends_with("<stdout>") {
      crate::display::write_to_stdout_ignore_sigpipe(bytes.as_slice())?;
      continue;
    }

    if let Some(parent) = path.parent()
      && !exists_cache.contains(parent)
    {
      if !parent.exists() {
        std::fs::create_dir_all(parent)?;
      }
      exists_cache.insert(parent.to_path_buf());
    }

    output_infos.push(OutputFileInfo {
      relative_path,
      size: bytes.len(),
      is_js,
    });

    std::fs::write(path, bytes.as_ref())?;
  }
  Ok(output_infos)
}

fn print_finished_message(
  metafile: &esbuild_client::Metafile,
  output_infos: &[OutputFileInfo],
  duration: Duration,
) -> Result<(), AnyError> {
  let mut output = String::new();
  output.push_str(&format!(
    "{} {} module{} in {}",
    deno_terminal::colors::green("Bundled"),
    metafile.inputs.len(),
    if metafile.inputs.len() == 1 { "" } else { "s" },
    crate::display::human_elapsed(duration.as_millis()),
  ));

  let longest = output_infos
    .iter()
    .map(|info| info.relative_path.to_string_lossy().len())
    .max()
    .unwrap_or(0);
  for info in output_infos {
    output.push_str(&format!(
      "\n  {} {}",
      if info.is_js {
        deno_terminal::colors::cyan(format!(
          "{:<longest$}",
          info.relative_path.display()
        ))
      } else {
        deno_terminal::colors::magenta(format!(
          "{:<longest$}",
          info.relative_path.display()
        ))
      },
      deno_terminal::colors::gray(
        crate::display::human_size(info.size as f64,)
      )
    ));
  }
  output.push('\n');
  log::info!("{}", output);

  Ok(())
}
