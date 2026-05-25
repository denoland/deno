// Copyright 2018-2026 the Deno authors. MIT license.

mod externals;
mod html;
mod provider;
mod transform;

use std::borrow::Cow;
use std::fmt::Debug;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use arcstr::ArcStr;
use deno_ast::EmitOptions;
use deno_ast::MediaType;
use deno_ast::ModuleKind;
use deno_ast::ModuleSpecifier;
use deno_bundle_runtime::BundleFormat;
use deno_bundle_runtime::BundlePlatform;
use deno_bundle_runtime::SourceMapType;
use deno_config::workspace::TsTypeLib;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt as _;
use deno_core::parking_lot::RwLock;
use deno_core::url::Url;
use deno_error::JsErrorClass;
use deno_graph::Position;
use deno_path_util::resolve_url_or_path;
use deno_resolver::cache::ParsedSourceCache;
use deno_resolver::graph::ResolveWithGraphError;
use deno_resolver::graph::ResolveWithGraphOptions;
use deno_resolver::loader::LoadCodeSourceError;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageReqReference;
use indexmap::IndexMap;
use indexmap::IndexSet;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use node_resolver::errors::PackageNotFoundError;
pub use provider::CliBundleProvider;
use rolldown::BundleOutput;
use rolldown::BundlerBuilder;
use rolldown::BundlerOptions;
use rolldown::InputItem;
use rolldown::OutputFormat;
use rolldown::Platform;
use rolldown::plugin::__inner::SharedPluginable;
use rolldown::plugin::HookLoadArgs;
use rolldown::plugin::HookLoadOutput;
use rolldown::plugin::HookResolveIdArgs;
use rolldown::plugin::HookResolveIdOutput;
use rolldown::plugin::HookUsage;
use rolldown::plugin::Plugin;
use rolldown_common::ImportKind;
use rolldown_common::ModuleType;
use rolldown_common::Output;
use rolldown_common::ResolvedExternal;
use rolldown_common::SourceMapType as RolldownSourceMapType;
use rolldown_error::Severity;

use crate::args::BundleFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::file_fetcher::CliFileFetcher;
use crate::graph_container::MainModuleGraphContainer;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::module_loader::CliEmitter;
use crate::module_loader::ModuleLoadPreparer;
use crate::module_loader::PrepareModuleLoadOptions;
use crate::node::CliNodeResolver;
use crate::npm::CliNpmResolver;
use crate::resolver::CliCjsTracker;
use crate::resolver::CliResolver;
use crate::tools::bundle::externals::ExternalsMatcher;
use crate::util::file_watcher::WatcherRestartMode;
use crate::util::fs::canonicalize_path;

pub async fn prepare_inputs(
  resolver: &CliResolver,
  npm_resolver: &CliNpmResolver,
  node_resolver: &CliNodeResolver,
  init_cwd: &Path,
  bundle_flags: &BundleFlags,
  plugin_handler: &mut DenoPluginHandler,
) -> Result<BundlerInput, AnyError> {
  let resolved_entrypoints =
    resolve_entrypoints(resolver, init_cwd, &bundle_flags.entrypoints)?;

  // Partition into HTML and non-HTML entrypoints
  let mut html_paths = Vec::new();
  let mut script_entry_urls = Vec::new();
  for url in &resolved_entrypoints {
    if url.as_str().to_lowercase().ends_with(".html") {
      let path = deno_path_util::url_to_file_path(url)?;
      html_paths.push(path);
    } else {
      script_entry_urls.push(url.clone());
    }
  }

  if html_paths.is_empty() {
    plugin_handler
      .prepare_module_load(&resolved_entrypoints)
      .await?;

    let roots =
      resolve_roots(script_entry_urls, init_cwd, npm_resolver, node_resolver);

    plugin_handler.prepare_module_load(&roots).await?;
    let graph = plugin_handler.module_graph_container.graph();
    let mut fully_resolved_roots = IndexSet::with_capacity(graph.roots.len());
    for root in &graph.roots {
      fully_resolved_roots.insert(graph.resolve(root).clone());
    }
    *plugin_handler.resolved_roots.write() = Arc::new(fully_resolved_roots);

    let entries: Vec<(String, String)> = roots
      .iter()
      .map(|url| {
        let path = file_path_or_url(url.clone()).unwrap();
        let name = entry_name_for_url(url, init_cwd);
        (name, path)
      })
      .collect();

    Ok(BundlerInput::Entrypoints(entries))
  } else {
    let virtual_modules = Arc::new(VirtualModules::new());
    plugin_handler.virtual_modules = Some(virtual_modules.clone());
    let mut html_entrypoints = Vec::new();
    let mut all_entries = Vec::new();

    for html_path in &html_paths {
      let html_entry = html::load_html_entrypoint(init_cwd, html_path)?;
      let virtual_module_url =
        deno_path_util::url_from_file_path(&html_entry.virtual_module_path)?
          .to_string();
      virtual_modules.insert(
        virtual_module_url.clone(),
        VirtualModule::new(
          html_entry.temp_module.as_bytes().to_vec(),
          ModuleType::Js,
        ),
      );
      all_entries.push((String::new(), virtual_module_url));
      html_entrypoints.push(html_entry);
    }

    let _ = plugin_handler.prepare_module_load(&script_entry_urls).await;

    let roots =
      resolve_roots(script_entry_urls, init_cwd, npm_resolver, node_resolver);
    let _ = plugin_handler.prepare_module_load(&roots).await;

    let to_cache_urls: Vec<Url> = all_entries
      .iter()
      .filter_map(|(_, url)| Url::parse(url).ok())
      .collect();
    let _ = plugin_handler.prepare_module_load(&to_cache_urls).await;

    let graph = plugin_handler.module_graph_container.graph();
    let mut fully_resolved_roots = IndexSet::with_capacity(graph.roots.len());
    for root in &graph.roots {
      fully_resolved_roots.insert(graph.resolve(root).clone());
    }
    *plugin_handler.resolved_roots.write() = Arc::new(fully_resolved_roots);

    Ok(BundlerInput::EntrypointsWithHtml {
      entries: all_entries,
      html_pages: html_entrypoints,
    })
  }
}

pub async fn bundle_init(
  mut flags: Arc<Flags>,
  bundle_flags: &BundleFlags,
) -> Result<RolldownBundler, AnyError> {
  {
    let flags_mut = Arc::make_mut(&mut flags);
    flags_mut.unstable_config.sloppy_imports = true;
  }
  let factory = CliFactory::from_flags(flags.clone());

  let resolver = factory.resolver().await?.clone();
  let module_load_preparer = factory.module_load_preparer().await?.clone();
  let root_permissions = factory.root_permissions_container()?;
  let npm_resolver = factory.npm_resolver().await?;
  let node_resolver = factory.node_resolver().await?;
  let cli_options = factory.cli_options()?;
  let init_cwd = cli_options.initial_cwd().to_path_buf();
  let module_graph_container =
    factory.main_module_graph_container().await?.clone();

  let mut plugin_handler = DenoPluginHandler {
    file_fetcher: factory.file_fetcher()?.clone(),
    resolver: resolver.clone(),
    module_load_preparer,
    resolved_roots: Arc::new(RwLock::new(Arc::new(IndexSet::new()))),
    module_graph_container,
    permissions: root_permissions.clone(),
    externals_matcher: if bundle_flags.external.is_empty() {
      None
    } else {
      Some(Arc::new(ExternalsMatcher::new(
        &bundle_flags.external,
        &init_cwd,
      )))
    },
    parsed_source_cache: factory.parsed_source_cache()?.clone(),
    cjs_tracker: factory.cjs_tracker()?.clone(),
    emitter: factory.emitter()?.clone(),
    virtual_modules: None,
    initial_cwd: deno_path_util::url_from_directory_path(
      cli_options.initial_cwd(),
    )?,
  };

  let input = prepare_inputs(
    &resolver,
    npm_resolver,
    node_resolver,
    &init_cwd,
    bundle_flags,
    &mut plugin_handler,
  )
  .await?;

  let entries = match &input {
    BundlerInput::Entrypoints(entries) => entries.clone(),
    BundlerInput::EntrypointsWithHtml { entries, .. } => entries.clone(),
  };

  let is_html = matches!(input, BundlerInput::EntrypointsWithHtml { .. });
  let rolldown_options =
    build_rolldown_options(bundle_flags, &init_cwd, is_html, &entries);

  let bundler = RolldownBundler {
    options: rolldown_options,
    plugin: Arc::new(plugin_handler),
    cwd: init_cwd,
    input,
    minified: bundle_flags.minify,
    platform: bundle_flags.platform,
  };

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
  let output = bundler.build().await?;
  let end = std::time::Instant::now();
  let duration = end.duration_since(start);

  if bundle_flags.watch {
    handle_diagnostics(&output);
    if has_errors(&output) {
      deno_core::anyhow::bail!("bundling failed");
    }
    return bundle_watch(
      flags,
      bundler,
      bundle_flags.output_dir.as_ref().map(Path::new),
      bundle_flags.output_path.as_ref().map(Path::new),
    )
    .await;
  }

  handle_diagnostics(&output);

  if !has_errors(&output) {
    let output_infos = process_result(
      &output,
      &init_cwd,
      bundle_flags.output_dir.as_ref().map(Path::new),
      bundle_flags.output_path.as_ref().map(Path::new),
      should_replace_require_shim(bundle_flags.platform),
      bundle_flags.minify,
      Some(&bundler.input),
    )?;

    if bundle_flags.output_dir.is_some() || bundle_flags.output_path.is_some() {
      print_finished_message(&output, &output_infos, duration)?;
    }
  }

  if has_errors(&output) {
    deno_core::anyhow::bail!("bundling failed");
  }

  Ok(())
}

async fn bundle_watch(
  flags: Arc<Flags>,
  bundler: RolldownBundler,
  output_dir: Option<&Path>,
  output_path: Option<&Path>,
) -> Result<(), AnyError> {
  let (initial_roots, always_watch) = match &bundler.input {
    BundlerInput::Entrypoints(entries) => (
      entries
        .iter()
        .filter_map(|(_, root)| {
          let url = Url::parse(root).ok()?;
          deno_path_util::url_to_file_path(&url).ok()
        })
        .collect::<Vec<_>>(),
      vec![],
    ),
    BundlerInput::EntrypointsWithHtml {
      entries,
      html_pages,
    } => {
      let mut roots = entries
        .iter()
        .filter_map(|(_, root)| {
          let url = Url::parse(root).ok()?;
          deno_path_util::url_to_file_path(&url).ok()
        })
        .collect::<Vec<_>>();
      let always = html_pages
        .iter()
        .map(|p| p.path.clone())
        .collect::<Vec<_>>();
      roots.extend(always.iter().cloned());
      (roots, always)
    }
  };
  let always_watch = Rc::new(always_watch);
  let current_roots = Rc::new(std::cell::RefCell::new(initial_roots.clone()));
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
      let always_watch = always_watch.clone();
      Ok(async move {
        let bundler = bundler.lock().await;
        let start = std::time::Instant::now();
        let output = bundler.build().await?;
        handle_diagnostics(&output);
        if !has_errors(&output) {
          let output_infos = process_result(
            &output,
            &bundler.cwd,
            output_dir,
            output_path,
            should_replace_require_shim(bundler.platform),
            bundler.minified,
            Some(&bundler.input),
          )?;
          print_finished_message(&output, &output_infos, start.elapsed())?;

          let mut new_watched = get_input_paths_from_output(&output);
          new_watched.extend(always_watch.iter().cloned());
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

fn get_input_paths_from_output(output: &BundleOutput) -> Vec<PathBuf> {
  let mut paths = IndexSet::new();
  for asset in &output.assets {
    if let Output::Chunk(chunk) = asset {
      for module_id in &chunk.module_ids {
        let module_str: &str = module_id.as_str();
        if let Ok(url) = Url::parse(module_str) {
          if let Ok(path) = deno_path_util::url_to_file_path(&url) {
            paths.insert(path);
          }
        } else if let Ok(path) = canonicalize_path(Path::new(module_str)) {
          paths.insert(path);
        }
      }
    }
  }
  paths.into_iter().collect()
}

// --- Bundler ---

#[derive(Debug, Clone)]
pub enum BundlerInput {
  Entrypoints(Vec<(String, String)>),
  EntrypointsWithHtml {
    entries: Vec<(String, String)>,
    html_pages: Vec<html::HtmlEntrypoint>,
  },
}

pub struct RolldownBundler {
  options: BundlerOptions,
  plugin: Arc<DenoPluginHandler>,
  pub cwd: PathBuf,
  pub input: BundlerInput,
  pub minified: bool,
  pub platform: BundlePlatform,
}

impl RolldownBundler {
  pub async fn build(&self) -> Result<BundleOutput, AnyError> {
    let plugin: SharedPluginable = self.plugin.clone();
    let mut bundler = BundlerBuilder::default()
      .with_options(self.options.clone())
      .with_plugins(vec![plugin])
      .build()
      .map_err(|errs| {
        let msg = errs
          .iter()
          .map(|e| e.to_string())
          .collect::<Vec<_>>()
          .join("\n");
        deno_core::anyhow::anyhow!("Failed to initialize bundler: {}", msg)
      })?;

    let output = bundler.generate().await.map_err(|errs| {
      let msg = errs
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n");
      deno_core::anyhow::anyhow!("Bundling failed: {}", msg)
    })?;

    Ok(output)
  }
}

// --- Configuration ---

fn build_rolldown_options(
  bundle_flags: &BundleFlags,
  cwd: &Path,
  is_html: bool,
  entries: &[(String, String)],
) -> BundlerOptions {
  let input: Vec<InputItem> = entries
    .iter()
    .map(|(name, import)| InputItem {
      name: if name.is_empty() {
        None
      } else {
        Some(name.clone())
      },
      import: import.clone(),
    })
    .collect();

  let format = Some(match bundle_flags.format {
    BundleFormat::Esm => OutputFormat::Esm,
    BundleFormat::Cjs => OutputFormat::Cjs,
    BundleFormat::Iife => OutputFormat::Iife,
  });

  let sourcemap = bundle_flags.sourcemap.map(|sm| match sm {
    SourceMapType::Linked => RolldownSourceMapType::File,
    SourceMapType::Inline => RolldownSourceMapType::Inline,
    SourceMapType::External => RolldownSourceMapType::Hidden,
  });

  let platform = match bundle_flags.platform {
    BundlePlatform::Browser => Some(Platform::Browser),
    BundlePlatform::Deno => Some(Platform::Neutral),
  };

  let mut options = BundlerOptions {
    input: Some(input),
    cwd: Some(cwd.to_path_buf()),
    format,
    sourcemap,
    platform,
    ..Default::default()
  };

  if bundle_flags.minify {
    // Enable minification — Rolldown uses Oxc minifier
    options.minify = Some(true.into());
  }

  if bundle_flags.keep_names {
    options.keep_names = Some(true);
  }

  if let Some(outdir) = &bundle_flags.output_dir {
    options.dir = Some(outdir.clone());
  } else if let Some(output_path) = &bundle_flags.output_path {
    options.file = Some(output_path.clone());
  }

  if is_html {
    options.platform = Some(Platform::Browser);
    options.entry_filenames = Some("[name]-[hash].js".to_string().into());
    options.chunk_filenames = Some("[name]-[hash].js".to_string().into());
    options.asset_filenames = Some("[name]-[hash][extname]".to_string().into());
  }

  options
}

// --- Plugin ---

#[derive(Clone)]
pub struct VirtualModule {
  contents: Vec<u8>,
  module_type: ModuleType,
}

impl VirtualModule {
  pub fn new(contents: Vec<u8>, module_type: ModuleType) -> Self {
    Self {
      contents,
      module_type,
    }
  }
}

pub struct VirtualModules {
  modules: RwLock<IndexMap<String, VirtualModule>>,
}

impl VirtualModules {
  pub fn new() -> Self {
    Self {
      modules: RwLock::new(IndexMap::new()),
    }
  }

  pub fn insert(&self, path: String, contents: VirtualModule) {
    self.modules.write().insert(path, contents);
  }

  pub fn get(&self, path: &str) -> Option<VirtualModule> {
    self.modules.read().get(path).cloned()
  }

  pub fn contains(&self, path: &str) -> bool {
    self.modules.read().contains_key(path)
  }
}

pub struct DenoPluginHandler {
  file_fetcher: Arc<CliFileFetcher>,
  resolver: Arc<CliResolver>,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  resolved_roots: Arc<RwLock<Arc<IndexSet<ModuleSpecifier>>>>,
  module_graph_container: Arc<MainModuleGraphContainer>,
  permissions: PermissionsContainer,
  externals_matcher: Option<Arc<ExternalsMatcher>>,
  virtual_modules: Option<Arc<VirtualModules>>,
  parsed_source_cache: Arc<ParsedSourceCache>,
  cjs_tracker: Arc<CliCjsTracker>,
  emitter: Arc<CliEmitter>,
  initial_cwd: Url,
}

impl Debug for DenoPluginHandler {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("DenoPluginHandler")
      .field("initial_cwd", &self.initial_cwd)
      .finish()
  }
}

impl Plugin for DenoPluginHandler {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("deno")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }

  fn resolve_id(
    &self,
    _ctx: &rolldown::plugin::PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> impl std::future::Future<
    Output = deno_core::anyhow::Result<Option<HookResolveIdOutput>>,
  > + Send {
    let specifier = args.specifier.to_string();
    let importer = args.importer.map(|s| s.to_string());
    let kind = args.kind;

    // We need to capture self's fields since the future must be Send
    let resolver = self.resolver.clone();
    let externals_matcher = self.externals_matcher.clone();
    let virtual_modules = self.virtual_modules.clone();
    let module_graph_container = self.module_graph_container.clone();
    let initial_cwd = self.initial_cwd.clone();

    async move {
      // Virtual module
      if let Some(vm) = &virtual_modules
        && vm.contains(&specifier)
      {
        return Ok(Some(HookResolveIdOutput {
          id: ArcStr::from(specifier),
          ..Default::default()
        }));
      }

      // Pre-resolve external check
      if let Some(matcher) = &externals_matcher
        && matcher.is_pre_resolve_match(&specifier)
      {
        return Ok(Some(HookResolveIdOutput {
          id: ArcStr::from(specifier),
          external: Some(ResolvedExternal::Bool(true)),
          ..Default::default()
        }));
      }

      // data: URLs pass through
      if specifier.starts_with("data:") {
        return Ok(None);
      }

      // Same-document fragment references in CSS — `url(#default#VML)`,
      // `url(#gradient)` for SVG paint servers, etc. — must be left as-is.
      // See denoland/deno#32232.
      if matches!(kind, ImportKind::UrlImport | ImportKind::AtImport)
        && specifier.starts_with('#')
      {
        return Ok(Some(HookResolveIdOutput {
          id: ArcStr::from(specifier),
          external: Some(ResolvedExternal::Bool(true)),
          ..Default::default()
        }));
      }

      // Resolve using Deno's resolver
      let referrer = if let Some(imp) = &importer {
        resolve_url_or_path(imp, Path::new(""))
          .unwrap_or_else(|_| initial_cwd.clone())
      } else {
        initial_cwd.clone()
      };

      let resolution_mode = match kind {
        ImportKind::Require => ResolutionMode::Require,
        _ => ResolutionMode::Import,
      };

      let graph = module_graph_container.graph();
      let result = resolver.resolve_with_graph(
        &graph,
        &specifier,
        &referrer,
        Position::new(0, 0),
        ResolveWithGraphOptions {
          mode: resolution_mode,
          kind: NodeResolutionKind::Execution,
          maintain_npm_specifiers: false,
        },
      );

      match result {
        Ok(resolved) => {
          let resolved_str = file_path_or_url(resolved)?;

          let is_post_external = externals_matcher
            .as_ref()
            .map(|m| m.is_post_resolve_match(&resolved_str))
            .unwrap_or(false);
          let is_external = is_post_external
            || resolved_str.starts_with("node:")
            || resolved_str.starts_with("bun:");

          Ok(Some(HookResolveIdOutput {
            id: ArcStr::from(resolved_str),
            external: if is_external {
              Some(ResolvedExternal::Bool(true))
            } else {
              None
            },
            ..Default::default()
          }))
        }
        Err(e) => {
          if maybe_ignorable_resolution_error(&e).is_some() {
            return Ok(None);
          }
          Err(e.into())
        }
      }
    }
  }

  fn load(
    &self,
    _ctx: rolldown::plugin::SharedLoadPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> impl std::future::Future<
    Output = deno_core::anyhow::Result<Option<HookLoadOutput>>,
  > + Send {
    let id = args.id.to_string();
    let virtual_modules = self.virtual_modules.clone();
    let file_fetcher = self.file_fetcher.clone();
    let resolver = self.resolver.clone();
    let module_graph_container = self.module_graph_container.clone();
    let permissions = self.permissions.clone();
    let parsed_source_cache = self.parsed_source_cache.clone();
    let cjs_tracker = self.cjs_tracker.clone();
    let emitter = self.emitter.clone();
    let resolved_roots = self.resolved_roots.clone();

    async move {
      // Virtual module
      if let Some(vm) = &virtual_modules
        && let Some(module) = vm.get(&id)
      {
        let code = String::from_utf8(module.contents)?;
        return Ok(Some(HookLoadOutput {
          code: ArcStr::from(code),
          module_type: Some(module.module_type.clone()),
          ..Default::default()
        }));
      }

      // Parse URL
      let specifier = if let Ok(url) = Url::parse(&id) {
        url
      } else {
        deno_path_util::url_from_file_path(Path::new(&id))?
      };

      // Look up in the module graph
      let graph = module_graph_container.graph();
      let module = graph.get(&specifier);

      let (resolved_specifier, media_type) = match module {
        Some(deno_graph::Module::Js(js)) => {
          (js.specifier.clone(), js.media_type)
        }
        Some(deno_graph::Module::Json(json)) => {
          (json.specifier.clone(), MediaType::Json)
        }
        Some(deno_graph::Module::Wasm(_)) => {
          return Err(deno_core::anyhow::anyhow!(
            "Wasm modules are not yet implemented in deno bundle."
          ));
        }
        Some(deno_graph::Module::Npm(_)) => {
          let req_ref = NpmPackageReqReference::from_specifier(&specifier)?;
          let url = resolver.resolve_managed_npm_req_ref(
            &req_ref,
            None,
            ResolutionMode::Import,
            NodeResolutionKind::Execution,
          )?;
          let (mt, _) =
            deno_media_type::resolve_media_type_and_charset_from_content_type(
              &url, None,
            );
          (url, mt)
        }
        Some(deno_graph::Module::Node(_) | deno_graph::Module::External(_)) => {
          return Ok(None);
        }
        None => {
          // Not in graph — try reading directly
          if specifier.scheme() == "file" {
            let path = deno_path_util::url_to_file_path(&specifier)?;
            match tokio::fs::read_to_string(&path).await {
              Ok(source) => {
                let (mt, _) =
                  deno_media_type::resolve_media_type_and_charset_from_content_type(
                    &specifier, None,
                  );
                let module_type = media_type_to_module_type(mt);

                if needs_transpile(mt) {
                  let source_arc: Arc<str> = Arc::from(source.as_str());
                  let parsed_source = parsed_source_cache
                    .remove_or_parse_module(&specifier, mt, source_arc)?;
                  let is_cjs = cjs_tracker.is_maybe_cjs(&specifier, mt)?
                    && parsed_source.compute_is_script();
                  let module_kind = ModuleKind::from_is_cjs(is_cjs);
                  let transpiled = emitter
                    .maybe_emit_parsed_source(parsed_source, module_kind)
                    .await?;
                  return Ok(Some(HookLoadOutput {
                    code: ArcStr::from(transpiled.as_ref()),
                    module_type: Some(ModuleType::Js),
                    ..Default::default()
                  }));
                }

                return Ok(Some(HookLoadOutput {
                  code: ArcStr::from(source.as_str()),
                  module_type: Some(module_type),
                  ..Default::default()
                }));
              }
              Err(_) => return Ok(None),
            }
          } else if matches!(specifier.scheme(), "http" | "https") {
            // Remote module not in graph — try to fetch via Deno's file
            // fetcher to trigger the right permission-check error message.
            let (tx, rx) = tokio::sync::oneshot::channel();
            let ff = file_fetcher.clone();
            let spec = specifier.clone();
            let perms = permissions.clone();
            deno_core::unsync::spawn(async move {
              let result = ff.fetch(&spec, &perms).await;
              let _ = tx.send(result);
            });
            let fetched =
              rx.await?.map_err(|e| deno_core::anyhow::anyhow!("{}", e))?;
            let source = String::from_utf8(fetched.source.to_vec())?;
            let (mt, _) =
              deno_media_type::resolve_media_type_and_charset_from_content_type(
                &specifier, None,
              );
            let module_type = media_type_to_module_type(mt);
            return Ok(Some(HookLoadOutput {
              code: ArcStr::from(source.as_str()),
              module_type: Some(module_type),
              ..Default::default()
            }));
          } else {
            return Ok(None);
          }
        }
      };

      // Load module source — read file directly for Send compatibility
      let source_string = if resolved_specifier.scheme() == "file" {
        let path = deno_path_util::url_to_file_path(&resolved_specifier)?;
        tokio::fs::read_to_string(&path).await.map_err(|e| {
          deno_core::anyhow::anyhow!("Failed to read {}: {}", path.display(), e)
        })?
      } else {
        // For remote modules, use a oneshot channel to bridge non-Send fetch
        let (tx, rx) = tokio::sync::oneshot::channel();
        let ff = file_fetcher.clone();
        let spec = resolved_specifier.clone();
        let perms = permissions.clone();
        deno_core::unsync::spawn(async move {
          let result = ff.fetch(&spec, &perms).await;
          let _ = tx.send(result);
        });
        let fetched =
          rx.await?.map_err(|e| deno_core::anyhow::anyhow!("{}", e))?;
        String::from_utf8(fetched.source.to_vec())?
      };
      let source_bytes = source_string.as_bytes();
      let module_type = media_type_to_module_type(media_type);

      // Transpile TypeScript/JSX if needed
      if needs_transpile(media_type) {
        let source_str = std::str::from_utf8(source_bytes)?;
        let source_arc: Arc<str> = Arc::from(source_str);
        let parsed_source = parsed_source_cache.remove_or_parse_module(
          &resolved_specifier,
          media_type,
          source_arc.clone(),
        )?;

        let is_cjs = cjs_tracker
          .is_maybe_cjs(&resolved_specifier, media_type)?
          && parsed_source.compute_is_script();
        let module_kind = ModuleKind::from_is_cjs(is_cjs);
        let transpiled = emitter
          .maybe_emit_parsed_source(parsed_source, module_kind)
          .await?;

        // Apply import.meta.main transform for non-root modules
        let roots = resolved_roots.read().clone();
        if !graph.roots.contains(&resolved_specifier)
          && !roots.contains(&resolved_specifier)
        {
          let code = apply_transform(
            &roots,
            &module_graph_container,
            &resolved_specifier,
            media_type,
            &transpiled,
          )?;
          return Ok(Some(HookLoadOutput {
            code: ArcStr::from(code),
            module_type: Some(ModuleType::Js),
            ..Default::default()
          }));
        }

        return Ok(Some(HookLoadOutput {
          code: ArcStr::from(transpiled.as_ref()),
          module_type: Some(ModuleType::Js),
          ..Default::default()
        }));
      }

      // Non-transpile: return source as-is
      let source_str = std::str::from_utf8(source_bytes)?;
      Ok(Some(HookLoadOutput {
        code: ArcStr::from(source_str),
        module_type: Some(module_type),
        ..Default::default()
      }))
    }
  }
}

impl DenoPluginHandler {
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
          file_content_overrides: Default::default(),
        },
      )
      .await?;
    graph_permit.commit();
    Ok(())
  }

}

fn apply_transform(
  resolved_roots: &IndexSet<ModuleSpecifier>,
  module_graph_container: &MainModuleGraphContainer,
  specifier: &ModuleSpecifier,
  media_type: deno_ast::MediaType,
  code: &str,
) -> Result<String, BundleLoadError> {
  let graph = module_graph_container.graph();
  let mut xform = transform::BundleImportMetaMainTransform::new(
    graph.roots.contains(specifier) || resolved_roots.contains(specifier),
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
      xform.visit_mut_program(&mut program);
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

fn needs_transpile(media_type: MediaType) -> bool {
  matches!(
    media_type,
    MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Jsx
      | MediaType::Tsx
  )
}

fn media_type_to_module_type(media_type: MediaType) -> ModuleType {
  use deno_ast::MediaType::*;
  match media_type {
    JavaScript | Cjs | Mjs | Mts => ModuleType::Js,
    TypeScript | Cts | Dts | Dmts | Dcts => ModuleType::Js, // We transpile before returning
    Jsx | Tsx => ModuleType::Js, // We transpile before returning
    Css => ModuleType::Css,
    Json => ModuleType::Json,
    Jsonc | Json5 | Markdown | SourceMap | Html | Sql => ModuleType::Text,
    Wasm | Unknown => ModuleType::Binary,
  }
}

// --- Error types ---

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
    && let deno_graph::source::ResolveError::Other(other_err) =
      resolve_error.deref()
    && let Some(import_map_err) = other_err
      .get_ref()
      .downcast_ref::<import_map::ImportMapError>()
    && let import_map::ImportMapErrorKind::UnmappedBareSpecifier(..) =
      import_map_err.as_kind()
  {
    Some(specifier.to_string())
  } else {
    None
  }
}

#[derive(Debug, boxed_error::Boxed, deno_error::JsError)]
pub struct BundleLoadError(pub Box<BundleLoadErrorKind>);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum BundleLoadErrorKind {
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

// --- Output processing ---

fn has_errors(output: &BundleOutput) -> bool {
  output
    .warnings
    .iter()
    .any(|w| w.severity() == Severity::Error)
}

fn handle_diagnostics(output: &BundleOutput) {
  for diag in &output.warnings {
    match diag.severity() {
      Severity::Error => {
        log::error!("{}: {}", deno_terminal::colors::red_bold("error"), diag);
      }
      Severity::Warning => {
        log::warn!(
          "{}: {}",
          deno_terminal::colors::yellow("bundler warning"),
          diag
        );
      }
      Severity::Info => {
        log::info!("{}", diag);
      }
    }
  }
}

pub struct OutputFileInfo {
  relative_path: PathBuf,
  size: usize,
  is_js: bool,
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

#[derive(Debug)]
pub struct OutputFile<'a> {
  pub path: PathBuf,
  pub contents: Cow<'a, [u8]>,
}

pub fn process_result(
  output: &BundleOutput,
  cwd: &Path,
  outdir: Option<&Path>,
  output_path: Option<&Path>,
  should_replace_require: bool,
  minified: bool,
  input: Option<&BundlerInput>,
) -> Result<Vec<OutputFileInfo>, AnyError> {
  let mut exists_cache = std::collections::HashSet::new();
  let mut output_infos = Vec::new();

  // Build list of OutputFile entries with their target paths.
  let mut output_files: Vec<OutputFile> =
    Vec::with_capacity(output.assets.len());
  for asset in &output.assets {
    let filename = asset.filename();
    let path = if let Some(outdir) = outdir {
      let outdir = if outdir.is_absolute() {
        outdir.to_path_buf()
      } else {
        cwd.join(outdir)
      };
      outdir.join(filename)
    } else if let Some(output_path) = output_path {
      let abs_output_path = if output_path.is_absolute() {
        output_path.to_path_buf()
      } else {
        cwd.join(output_path)
      };
      if Path::new(filename).file_name() == abs_output_path.file_name() {
        abs_output_path
      } else if let Some(parent) = abs_output_path.parent() {
        parent.join(filename)
      } else {
        PathBuf::from(filename)
      }
    } else {
      cwd.join(filename)
    };
    output_files.push(OutputFile {
      path,
      contents: Cow::Borrowed(asset.content_as_bytes()),
    });
  }

  // For HTML entrypoints, patch each HTML page with the rolldown-generated
  // chunk filenames and append the patched HTML files to the output set.
  if let Some(BundlerInput::EntrypointsWithHtml { html_pages, .. }) = input {
    let outdir_path = outdir.map(|p| {
      if p.is_absolute() {
        p.to_path_buf()
      } else {
        cwd.join(p)
      }
    });
    if let Some(outdir) = outdir_path {
      let mut html_output_files = html::HtmlOutputFiles::new(&mut output_files);
      for page in html_pages {
        page.clone().patch_html_with_response(
          cwd,
          &outdir,
          &mut html_output_files,
        )?;
      }
    }
  }

  for file in output_files.iter() {
    let path = &file.path;
    let raw_bytes: &[u8] = &file.contents;
    let is_js_file = is_js(path);

    let processed: Option<Vec<u8>> = if is_js_file && should_replace_require {
      let s = std::str::from_utf8(raw_bytes)?;
      Some(replace_require_shim(s, minified).into_bytes())
    } else {
      None
    };
    let bytes: &[u8] = processed.as_deref().unwrap_or(raw_bytes);

    let relative_path =
      pathdiff::diff_paths(path, cwd).unwrap_or_else(|| path.clone());

    // If no output dir or path specified and single entry, write to stdout
    if outdir.is_none() && output_path.is_none() && output_files.len() == 1 {
      crate::display::write_to_stdout_ignore_sigpipe(bytes)?;
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
      is_js: is_js_file,
    });

    std::fs::write(path, bytes)?;
  }
  Ok(output_infos)
}

fn print_finished_message(
  output: &BundleOutput,
  output_infos: &[OutputFileInfo],
  duration: Duration,
) -> Result<(), AnyError> {
  // Count unique input modules across all chunks, excluding rolldown's
  // internal synthetic modules (e.g. `\0rolldown/runtime.js`).
  let mut input_ids = std::collections::HashSet::new();
  for asset in &output.assets {
    if let Output::Chunk(c) = asset {
      for id in &c.module_ids {
        let s = id.as_str();
        if !s.starts_with('\0') {
          input_ids.insert(s.to_string());
        }
      }
    }
  }
  let input_count = input_ids.len();

  let mut msg = String::new();
  msg.push_str(&format!(
    "{} {} module{} in {}",
    deno_terminal::colors::green("Bundled"),
    input_count,
    if input_count == 1 { "" } else { "s" },
    crate::display::human_elapsed(duration.as_millis()),
  ));

  let longest = output_infos
    .iter()
    .map(|info| info.relative_path.to_string_lossy().len())
    .max()
    .unwrap_or(0);
  for info in output_infos {
    msg.push_str(&format!(
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
  msg.push('\n');
  log::info!("{}", msg);

  Ok(())
}

// --- Utility functions ---

// Derives a chunk name preserving the subdirectory structure relative to cwd
// when possible. e.g. for `src/foo/main.ts` under `cwd`, returns `foo/main`.
// Falls back to just the basename for files outside cwd.
fn entry_name_for_url(url: &Url, cwd: &Path) -> String {
  if url.scheme() == "file"
    && let Ok(path) = deno_path_util::url_to_file_path(url)
  {
    // Strip the cwd prefix; if not under cwd, fall back to file_stem.
    let rel = path.strip_prefix(cwd).unwrap_or(&path);
    let mut buf = PathBuf::new();
    if let Some(parent) = rel.parent() {
      for comp in parent.components() {
        if let std::path::Component::Normal(c) = comp {
          // Skip a leading `src` directory, matching esbuild's default
          // behavior for common project layouts.
          if buf.as_os_str().is_empty() && c == std::ffi::OsStr::new("src") {
            continue;
          }
          buf.push(c);
        }
      }
    }
    if let Some(stem) = rel.file_stem() {
      buf.push(stem);
    }
    return buf.to_string_lossy().into_owned();
  }
  String::new()
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

fn resolve_url_or_path_absolute(
  specifier: &str,
  current_dir: &Path,
) -> Result<Url, AnyError> {
  if deno_path_util::specifier_has_uri_scheme(specifier) {
    Ok(Url::parse(specifier)?)
  } else {
    let path = current_dir.join(specifier);
    let path = deno_path_util::normalize_path(Cow::Owned(path));
    let path = canonicalize_path(&path)?;
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
  cwd: &Path,
  npm_resolver: &CliNpmResolver,
  node_resolver: &CliNodeResolver,
) -> Vec<Url> {
  let mut roots = Vec::with_capacity(entrypoints.len());

  for url in entrypoints {
    let root = match NpmPackageReqReference::from_specifier(&url) {
      Ok(v) => {
        let referrer = ModuleSpecifier::from_directory_path(cwd).unwrap();
        let package_folder = npm_resolver
          .resolve_pkg_folder_from_deno_module_req(v.req(), &referrer)
          .unwrap();
        let Ok(node_resolver::BinValue::JsFile(main_module)) =
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

pub fn should_replace_require_shim(platform: BundlePlatform) -> bool {
  matches!(platform, BundlePlatform::Deno)
}

// Rolldown emits a `__require` shim that uses a global `require` if available,
// or throws otherwise. When bundling for Deno (ESM, no global require), we
// replace it with `createRequire(import.meta.url)` from `node:module`.
fn replace_require_shim(contents: &str, minified: bool) -> String {
  if minified {
    // Rolldown's minified __require shim. Backticks `u` are oxc-minifier's
    // shorthand for the string "u".
    let re = lazy_regex::regex!(
      r#"(?P<prefix>var |,)(?P<name>\w+)=\(\w+=>typeof require<`u`\?require:typeof Proxy<`u`\?new Proxy\(\w+,\{get:\(\w+,\w+\)=>\(typeof require<`u`\?require:\w+\)\[\w+\]\}\):\w+\)\(function\(\w+\)\{if\(typeof require<`u`\)return require\.apply\(this,arguments\);throw Error\([^}]*\)\}\)(?P<suffix>;|,)"#
    );
    re.replace(contents, |c: &regex::Captures<'_>| {
      let prefix = c.name("prefix").unwrap().as_str();
      let name = c.name("name").unwrap().as_str();
      let suffix = c.name("suffix").unwrap().as_str();
      // Close the existing var statement, inject the createRequire import,
      // then start a new var statement.
      let close = if prefix == "," { ";" } else { "" };
      let open = if suffix == "," { "var " } else { "" };
      format!(
        "{close}import{{createRequire as __deno_internal_createRequire}}from\"node:module\";var {name}=__deno_internal_createRequire(import.meta.url);{open}"
      )
    }).into_owned()
  } else {
    let re = lazy_regex::regex!(
      r#"var __require = (/\* @__PURE__ \*/\s*)?\(\(\w+\) => typeof require !== "undefined" \? require : typeof Proxy !== "undefined" \? new Proxy\(\w+, \{\s*get: \(\w+, \w+\) => \(typeof require !== "undefined" \? require : \w+\)\[\w+\]\s*\}\) : \w+\)\(function\(\w+\) \{\s*if \(typeof require !== "undefined"\) return require\.apply\(this, arguments\);\s*throw Error\([^}]*\);\s*\}\);"#
    );
    re.replace_all(
      contents,
      r#"import { createRequire as __deno_internal_createRequire } from "node:module";
var __require = __deno_internal_createRequire(import.meta.url);"#,
    )
    .into_owned()
  }
}
