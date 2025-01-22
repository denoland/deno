// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::deno_json;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceDirectory;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_graph::Module;
use deno_graph::ModuleError;
use deno_graph::ModuleGraph;
use deno_graph::ModuleLoadError;
use deno_lib::util::hash::FastInsecureHasher;
use deno_semver::npm::NpmPackageNvReference;
use deno_terminal::colors;
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::args::check_warn_tsconfig;
use crate::args::CheckFlags;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::TsConfig;
use crate::args::TsConfigType;
use crate::args::TsTypeLib;
use crate::args::TypeCheckMode;
use crate::cache::CacheDBHash;
use crate::cache::Caches;
use crate::cache::TypeCheckCache;
use crate::factory::CliFactory;
use crate::graph_util::maybe_additional_sloppy_imports_message;
use crate::graph_util::BuildFastCheckGraphOptions;
use crate::graph_util::ModuleGraphBuilder;
use crate::node::CliNodeResolver;
use crate::npm::installer::NpmInstaller;
use crate::npm::CliNpmResolver;
use crate::sys::CliSys;
use crate::tsc;
use crate::tsc::Diagnostics;
use crate::tsc::TypeCheckingCjsTracker;
use crate::util::extract;
use crate::util::path::to_percent_decoded_str;

pub async fn check(
  flags: Arc<Flags>,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);

  let main_graph_container = factory.main_module_graph_container().await?;

  let specifiers =
    main_graph_container.collect_specifiers(&check_flags.files)?;
  if specifiers.is_empty() {
    log::warn!("{} No matching files found.", colors::yellow("Warning"));
  }

  let specifiers_for_typecheck = if check_flags.doc || check_flags.doc_only {
    let file_fetcher = factory.file_fetcher()?;
    let root_permissions = factory.root_permissions_container()?;

    let mut specifiers_for_typecheck = if check_flags.doc {
      specifiers.clone()
    } else {
      vec![]
    };

    for s in specifiers {
      let file = file_fetcher.fetch(&s, root_permissions).await?;
      let snippet_files = extract::extract_snippet_files(file)?;
      for snippet_file in snippet_files {
        specifiers_for_typecheck.push(snippet_file.url.clone());
        file_fetcher.insert_memory_files(snippet_file);
      }
    }

    specifiers_for_typecheck
  } else {
    specifiers
  };

  main_graph_container
    .check_specifiers(&specifiers_for_typecheck, None)
    .await
}

/// Options for performing a check of a module graph. Note that the decision to
/// emit or not is determined by the `ts_config` settings.
pub struct CheckOptions {
  /// Whether to build the fast check type graph if necessary.
  ///
  /// Note: For perf reasons, the fast check type graph is only
  /// built if type checking is necessary.
  pub build_fast_check_graph: bool,
  /// Default type library to type check with.
  pub lib: TsTypeLib,
  /// Whether to log about any ignored compiler options.
  pub log_ignored_options: bool,
  /// If true, valid `.tsbuildinfo` files will be ignored and type checking
  /// will always occur.
  pub reload: bool,
  /// Mode to type check with.
  pub type_check_mode: TypeCheckMode,
}

pub struct TypeChecker {
  caches: Arc<Caches>,
  cjs_tracker: Arc<TypeCheckingCjsTracker>,
  cli_options: Arc<CliOptions>,
  module_graph_builder: Arc<ModuleGraphBuilder>,
  npm_installer: Option<Arc<NpmInstaller>>,
  node_resolver: Arc<CliNodeResolver>,
  npm_resolver: CliNpmResolver,
  sys: CliSys,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CheckError {
  #[class(inherit)]
  #[error(transparent)]
  Diagnostics(#[from] Diagnostics),
  #[class(inherit)]
  #[error(transparent)]
  ConfigFile(#[from] deno_json::ConfigFileError),
  #[class(inherit)]
  #[error(transparent)]
  ToMaybeJsxImportSourceConfig(
    #[from] deno_json::ToMaybeJsxImportSourceConfigError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  TscExec(#[from] tsc::ExecError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

impl TypeChecker {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    caches: Arc<Caches>,
    cjs_tracker: Arc<TypeCheckingCjsTracker>,
    cli_options: Arc<CliOptions>,
    module_graph_builder: Arc<ModuleGraphBuilder>,
    node_resolver: Arc<CliNodeResolver>,
    npm_installer: Option<Arc<NpmInstaller>>,
    npm_resolver: CliNpmResolver,
    sys: CliSys,
  ) -> Self {
    Self {
      caches,
      cjs_tracker,
      cli_options,
      module_graph_builder,
      node_resolver,
      npm_installer,
      npm_resolver,
      sys,
    }
  }

  /// Type check the module graph.
  ///
  /// It is expected that it is determined if a check and/or emit is validated
  /// before the function is called.
  pub async fn check(
    &self,
    graph: ModuleGraph,
    options: CheckOptions,
  ) -> Result<Arc<ModuleGraph>, CheckError> {
    let (graph, mut diagnostics) =
      self.check_diagnostics(graph, options).await?;
    diagnostics.emit_warnings();
    if diagnostics.is_empty() {
      Ok(graph)
    } else {
      Err(diagnostics.into())
    }
  }

  /// Type check the module graph returning its diagnostics.
  ///
  /// It is expected that it is determined if a check and/or emit is validated
  /// before the function is called.
  pub async fn check_diagnostics(
    &self,
    mut graph: ModuleGraph,
    options: CheckOptions,
  ) -> Result<(Arc<ModuleGraph>, Diagnostics), CheckError> {
    fn check_state_hash(resolver: &CliNpmResolver) -> Option<u64> {
      match resolver {
        CliNpmResolver::Byonm(_) => {
          // not feasible and probably slower to compute
          None
        }
        CliNpmResolver::Managed(resolver) => {
          // we should probably go further and check all the individual npm packages
          let mut package_reqs = resolver.resolution().package_reqs();
          package_reqs.sort_by(|a, b| a.0.cmp(&b.0)); // determinism
          let mut hasher = FastInsecureHasher::new_without_deno_version();
          // ensure the cache gets busted when turning nodeModulesDir on or off
          // as this could cause changes in resolution
          hasher.write_hashable(resolver.root_node_modules_path().is_some());
          for (pkg_req, pkg_nv) in package_reqs {
            hasher.write_hashable(&pkg_req);
            hasher.write_hashable(&pkg_nv);
          }
          Some(hasher.finish())
        }
      }
    }

    if !options.type_check_mode.is_true() || graph.roots.is_empty() {
      return Ok((graph.into(), Default::default()));
    }

    // node built-in specifiers use the @types/node package to determine
    // types, so inject that now (the caller should do this after the lockfile
    // has been written)
    if let Some(npm_installer) = &self.npm_installer {
      if graph.has_node_specifier {
        npm_installer.inject_synthetic_types_node_package().await?;
      }
    }

    // add fast check to the graph before getting the roots
    if options.build_fast_check_graph {
      self.module_graph_builder.build_fast_check_graph(
        &mut graph,
        BuildFastCheckGraphOptions {
          workspace_fast_check: deno_graph::WorkspaceFastCheckOption::Disabled,
        },
      )?;
    }

    let graph = Arc::new(graph);

    let tsconfig_resolver = TsConfigResolver::from_workspace(
      self.cli_options.workspace(),
      options.lib,
      TsConfigResolverOptions {
        log_ignored_options: options.log_ignored_options,
      },
    )?;

    // collect the tsc roots
    let npm_cache_state_hash = check_state_hash(&self.npm_resolver);
    let TscRoots {
      roots: root_names,
      missing_diagnostics,
      maybe_hasher,
    } = get_tsc_roots(
      &self.sys,
      &self.npm_resolver,
      &self.node_resolver,
      &graph,
      &tsconfig_resolver,
      npm_cache_state_hash,
      options.type_check_mode,
      // &ts_config, // todo(THIS PR): handle this
    );

    let mut missing_diagnostics = missing_diagnostics
      .filter(|d| self.should_include_diagnostic(options.type_check_mode, d));
    missing_diagnostics.apply_fast_check_source_maps(&graph);

    // group every root into a folder
    struct FolderInfo<'a> {
      tsconfig: &'a TsConfig,
      roots: Vec<(Url, MediaType)>,
    }
    let mut root_names_by_config = IndexMap::new();
    for (url, media_type) in root_names {
      let folder = tsconfig_resolver.folder_for_specifier(&url);
      let entry = root_names_by_config
        .entry(folder.dir.dir_url())
        .or_insert_with(|| FolderInfo {
          roots: Vec::new(),
          tsconfig: &folder.tsconfig,
        });
      entry.roots.push((url, media_type));
    }

    let type_check_cache =
      TypeCheckCache::new(self.caches.type_checking_cache_db());
    let mut diagnostics = missing_diagnostics;
    let logged_check_roots = deno_core::unsync::Flag::lowered();
    let log_check_roots = || {
      if logged_check_roots.raise() {
        for root in &graph.roots {
          let root_str = root.as_str();
          log::info!(
            "{} {}",
            colors::green("Check"),
            to_percent_decoded_str(root_str)
          );
        }
      }
    };

    if !diagnostics.is_empty() {
      log_check_roots();
    }

    for (folder_url, folder) in root_names_by_config {
      let current_diagnostics = self.check_diagnostics_in_folder(
        folder.roots,
        folder.tsconfig,
        &graph,
        maybe_hasher.clone(),
        &type_check_cache,
        &options,
        &log_check_roots,
      )?;
      for diagnostic in current_diagnostics.into_iter() {
        if let Some(file_name) = &diagnostic.file_name {
          let folder = tsconfig_resolver.folder_for_specifier_str(file_name);
          if folder.dir.dir_url() != folder_url {
            continue; // not the diagnostic for this folder
          }
        }
        // ensure we don't insert a duplicate
        if !diagnostics.iter().any(|d| {
          d.code == diagnostic.code
            && d.file_name == diagnostic.file_name
            && d.start == diagnostic.start
            && d.end == diagnostic.end
        }) {
          diagnostics.push(diagnostic);
        }
      }
    }

    Ok((graph, diagnostics))
  }

  #[allow(clippy::too_many_arguments)]
  fn check_diagnostics_in_folder(
    &self,
    root_names: Vec<(Url, MediaType)>,
    ts_config: &TsConfig,
    graph: &Arc<ModuleGraph>,
    maybe_hasher: Option<FastInsecureHasher>,
    type_check_cache: &TypeCheckCache,
    options: &CheckOptions,
    log_check_roots: &impl Fn(),
  ) -> Result<Diagnostics, CheckError> {
    // the first root will always either be the specifier that the user provided
    // or the first specifier in a directory
    let first_root = root_names[0].0.clone();
    log::debug!("Type checking: {}", first_root);

    let type_check_mode = options.type_check_mode;
    let maybe_check_hash = maybe_hasher.map(|mut hasher| {
      CacheDBHash::new(
        hasher
          .write_str(first_root.as_str())
          .write_hashable(ts_config)
          .finish(),
      )
    });
    if !options.reload {
      // do not type check if we know this is type checked
      if let Some(check_hash) = maybe_check_hash {
        if type_check_cache.has_check_hash(check_hash) {
          log::debug!("Already type checked.");
          return Ok(Default::default());
        }
      }
    }

    log_check_roots();

    // while there might be multiple roots, we can't "merge" the build info, so we
    // try to retrieve the build info for first root, which is the most common use
    // case.
    let maybe_tsbuildinfo = if options.reload {
      None
    } else {
      type_check_cache.get_tsbuildinfo(&first_root)
    };
    // to make tsc build info work, we need to consistently hash modules, so that
    // tsc can better determine if an emit is still valid or not, so we provide
    // that data here.
    let tsconfig_hash_data = FastInsecureHasher::new_deno_versioned()
      .write_hashable(ts_config)
      .finish();
    let response = tsc::exec(tsc::Request {
      config: ts_config.clone(),
      debug: self.cli_options.log_level() == Some(log::Level::Debug),
      graph: graph.clone(),
      hash_data: tsconfig_hash_data,
      maybe_npm: Some(tsc::RequestNpmState {
        cjs_tracker: self.cjs_tracker.clone(),
        node_resolver: self.node_resolver.clone(),
        npm_resolver: self.npm_resolver.clone(),
      }),
      maybe_tsbuildinfo,
      root_names,
      check_mode: type_check_mode,
    })?;

    let mut diagnostics = response
      .diagnostics
      .filter(|d| self.should_include_diagnostic(options.type_check_mode, d));

    diagnostics.apply_fast_check_source_maps(graph);

    if let Some(tsbuildinfo) = response.maybe_tsbuildinfo {
      type_check_cache.set_tsbuildinfo(&first_root, &tsbuildinfo);
    }

    if diagnostics.is_empty() {
      if let Some(check_hash) = maybe_check_hash {
        type_check_cache.add_check_hash(check_hash);
      }
    }

    log::debug!("{}", response.stats);

    Ok(diagnostics)
  }

  fn should_include_diagnostic(
    &self,
    type_check_mode: TypeCheckMode,
    d: &tsc::Diagnostic,
  ) -> bool {
    if self.is_remote_diagnostic(d) {
      type_check_mode == TypeCheckMode::All && d.include_when_remote()
    } else {
      true
    }
  }

  fn is_remote_diagnostic(&self, d: &tsc::Diagnostic) -> bool {
    let Some(file_name) = &d.file_name else {
      return false;
    };
    if file_name.starts_with("https://") || file_name.starts_with("http://") {
      return true;
    }
    // check if in an npm package
    let Ok(specifier) = ModuleSpecifier::parse(file_name) else {
      return false;
    };
    self.node_resolver.in_npm_package(&specifier)
  }
}

struct TsConfigFolderInfo {
  dir: WorkspaceDirectory,
  tsconfig: TsConfig,
}

struct TsConfigResolverOptions {
  log_ignored_options: bool,
}

struct TsConfigResolver<'a> {
  unscoped: TsConfigFolderInfo,
  folders_by_tsconfig: BTreeMap<&'a Arc<Url>, TsConfigFolderInfo>,
}

impl<'a> TsConfigResolver<'a> {
  pub fn from_workspace(
    workspace: &'a Arc<Workspace>,
    lib: TsTypeLib,
    options: TsConfigResolverOptions,
  ) -> Result<Self, CheckError> {
    // separate the workspace into directories that have a tsconfig
    let mut folders_by_tsconfig = BTreeMap::new();
    for (url, folder) in workspace.config_folders() {
      let folder_has_compiler_options = folder
        .deno_json
        .as_ref()
        .map(|d| d.json.compiler_options.is_some())
        .unwrap_or(false);
      if url != workspace.root_dir() && folder_has_compiler_options {
        let dir = workspace.resolve_member_dir(url);
        let tsconfig_result =
          dir.to_ts_config_for_emit(TsConfigType::Check { lib })?;
        if options.log_ignored_options {
          check_warn_tsconfig(&tsconfig_result);
        }
        folders_by_tsconfig.insert(
          url,
          TsConfigFolderInfo {
            tsconfig: tsconfig_result.ts_config,
            dir,
          },
        );
      }
    }
    let root_dir = workspace.resolve_member_dir(workspace.root_dir());
    let tsconfig_result =
      root_dir.to_ts_config_for_emit(TsConfigType::Check { lib })?;
    if options.log_ignored_options {
      check_warn_tsconfig(&tsconfig_result);
    }
    Ok(Self {
      unscoped: TsConfigFolderInfo {
        tsconfig: tsconfig_result.ts_config,
        dir: root_dir,
      },
      folders_by_tsconfig,
    })
  }

  pub fn check_js_for_specifier(&self, specifier: &Url) -> bool {
    let folder = self.folder_for_specifier(specifier);
    folder.tsconfig.get_check_js()
  }

  pub fn folder_for_specifier(&self, specifier: &Url) -> &TsConfigFolderInfo {
    self.folder_for_specifier_str(specifier.as_str())
  }

  pub fn folder_for_specifier_str(
    &self,
    specifier: &str,
  ) -> &TsConfigFolderInfo {
    self
      .folders_by_tsconfig
      .iter()
      .rfind(|(s, _)| specifier.starts_with(s.as_str()))
      .map(|(_, v)| v)
      .unwrap_or(&self.unscoped)
  }
}

struct TscRoots {
  roots: Vec<(ModuleSpecifier, MediaType)>,
  missing_diagnostics: tsc::Diagnostics,
  // pending hasher that still doesn't have the tsconfig in it
  maybe_hasher: Option<FastInsecureHasher>,
}

/// Transform the graph into root specifiers that we can feed `tsc`. We have to
/// provide the media type for root modules because `tsc` does not "resolve" the
/// media type like other modules, as well as a root specifier needs any
/// redirects resolved. We need to include all the emittable files in
/// the roots, so they get type checked and optionally emitted,
/// otherwise they would be ignored if only imported into JavaScript.
#[allow(clippy::too_many_arguments)]
fn get_tsc_roots(
  sys: &CliSys,
  npm_resolver: &CliNpmResolver,
  node_resolver: &CliNodeResolver,
  graph: &ModuleGraph,
  tsconfig_resolver: &TsConfigResolver,
  npm_cache_state_hash: Option<u64>,
  type_check_mode: TypeCheckMode,
) -> TscRoots {
  fn maybe_get_check_entry(
    module: &deno_graph::Module,
    tsconfig_resolver: &TsConfigResolver,
    hasher: Option<&mut FastInsecureHasher>,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    match module {
      Module::Js(module) => {
        let result = match module.media_type {
          MediaType::TypeScript
          | MediaType::Tsx
          | MediaType::Mts
          | MediaType::Cts
          | MediaType::Dts
          | MediaType::Dmts
          | MediaType::Dcts => {
            Some((module.specifier.clone(), module.media_type))
          }
          MediaType::JavaScript
          | MediaType::Mjs
          | MediaType::Cjs
          | MediaType::Jsx => {
            if tsconfig_resolver.check_js_for_specifier(&module.specifier)
              || has_ts_check(module.media_type, &module.source)
            {
              Some((module.specifier.clone(), module.media_type))
            } else {
              None
            }
          }
          MediaType::Json
          | MediaType::Wasm
          | MediaType::Css
          | MediaType::SourceMap
          | MediaType::Unknown => None,
        };
        if result.is_some() {
          if let Some(hasher) = hasher {
            hasher.write_str(module.specifier.as_str());
            hasher.write_str(
              // the fast check module will only be set when publishing
              module
                .fast_check_module()
                .map(|s| s.source.as_ref())
                .unwrap_or(&module.source),
            );
          }
        }
        result
      }
      Module::Node(_) => {
        // the @types/node package will be in the resolved
        // snapshot so don't bother including it in the hash
        None
      }
      Module::Npm(_) => {
        // don't bother adding this specifier to the hash
        // because what matters is the resolved npm snapshot,
        // which is hashed below
        None
      }
      Module::Json(module) => {
        if let Some(hasher) = hasher {
          hasher.write_str(module.specifier.as_str());
          hasher.write_str(&module.source);
        }
        None
      }
      Module::Wasm(module) => {
        if let Some(hasher) = hasher {
          hasher.write_str(module.specifier.as_str());
          hasher.write_str(&module.source_dts);
        }
        Some((module.specifier.clone(), MediaType::Dmts))
      }
      Module::External(module) => {
        if let Some(hasher) = hasher {
          hasher.write_str(module.specifier.as_str());
        }

        None
      }
    }
  }

  let mut result = TscRoots {
    roots: Vec::with_capacity(graph.specifiers_count()),
    missing_diagnostics: Default::default(),
    maybe_hasher: None,
  };
  let mut maybe_hasher = npm_cache_state_hash.map(|npm_cache_state_hash| {
    let mut hasher = FastInsecureHasher::new_deno_versioned();
    hasher.write_hashable(npm_cache_state_hash);
    hasher.write_u8(match type_check_mode {
      TypeCheckMode::All => 0,
      TypeCheckMode::Local => 1,
      TypeCheckMode::None => 2,
    });
    hasher.write_hashable(graph.has_node_specifier);
    hasher
  });

  if graph.has_node_specifier {
    // inject a specifier that will resolve node types
    result.roots.push((
      ModuleSpecifier::parse("asset:///node_types.d.ts").unwrap(),
      MediaType::Dts,
    ));
  }

  let mut seen =
    HashSet::with_capacity(graph.imports.len() + graph.specifiers_count());
  let mut pending = VecDeque::new();

  for (referrer, import) in graph.imports.iter() {
    for specifier in import
      .dependencies
      .values()
      .filter_map(|dep| dep.get_type().or_else(|| dep.get_code()))
    {
      let specifier = graph.resolve(specifier);
      if seen.insert(specifier) {
        if let Ok(nv_ref) = NpmPackageNvReference::from_specifier(specifier) {
          let Some(resolved) =
            resolve_npm_nv_ref(npm_resolver, node_resolver, &nv_ref, referrer)
          else {
            result.missing_diagnostics.push(
              tsc::Diagnostic::from_missing_error(
                specifier,
                None,
                maybe_additional_sloppy_imports_message(sys, specifier),
              ),
            );
            continue;
          };
          let mt = MediaType::from_specifier(&resolved);
          result.roots.push((resolved, mt));
        } else {
          pending.push_back((specifier, false));
        }
      }
    }
  }

  // then the roots
  for root in &graph.roots {
    let specifier = graph.resolve(root);
    if seen.insert(specifier) {
      pending.push_back((specifier, false));
    }
  }

  // now walk the graph that only includes the fast check dependencies
  while let Some((specifier, is_dynamic)) = pending.pop_front() {
    let module = match graph.try_get(specifier) {
      Ok(Some(module)) => module,
      Ok(None) => continue,
      Err(ModuleError::Missing(specifier, maybe_range)) => {
        if !is_dynamic {
          result
            .missing_diagnostics
            .push(tsc::Diagnostic::from_missing_error(
              specifier,
              maybe_range.as_ref(),
              maybe_additional_sloppy_imports_message(sys, specifier),
            ));
        }
        continue;
      }
      Err(ModuleError::LoadingErr(
        specifier,
        maybe_range,
        ModuleLoadError::Loader(_),
      )) => {
        // these will be errors like attempting to load a directory
        if !is_dynamic {
          result
            .missing_diagnostics
            .push(tsc::Diagnostic::from_missing_error(
              specifier,
              maybe_range.as_ref(),
              maybe_additional_sloppy_imports_message(sys, specifier),
            ));
        }
        continue;
      }
      Err(_) => continue,
    };
    if is_dynamic && !seen.insert(specifier) {
      continue;
    }
    if let Some(entry) =
      maybe_get_check_entry(module, tsconfig_resolver, maybe_hasher.as_mut())
    {
      result.roots.push(entry);
    }

    let mut maybe_module_dependencies = None;
    let mut maybe_types_dependency = None;
    if let Module::Js(module) = module {
      maybe_module_dependencies = Some(module.dependencies_prefer_fast_check());
      maybe_types_dependency = module
        .maybe_types_dependency
        .as_ref()
        .and_then(|d| d.dependency.ok());
    } else if let Module::Wasm(module) = module {
      maybe_module_dependencies = Some(&module.dependencies);
    }

    fn handle_specifier<'a>(
      graph: &'a ModuleGraph,
      seen: &mut HashSet<&'a ModuleSpecifier>,
      pending: &mut VecDeque<(&'a ModuleSpecifier, bool)>,
      specifier: &'a ModuleSpecifier,
      is_dynamic: bool,
    ) {
      let specifier = graph.resolve(specifier);
      if is_dynamic {
        if !seen.contains(specifier) {
          pending.push_back((specifier, true));
        }
      } else if seen.insert(specifier) {
        pending.push_back((specifier, false));
      }
    }

    if let Some(deps) = maybe_module_dependencies {
      for dep in deps.values() {
        // walk both the code and type dependencies
        if let Some(specifier) = dep.get_code() {
          handle_specifier(
            graph,
            &mut seen,
            &mut pending,
            specifier,
            dep.is_dynamic,
          );
        }
        if let Some(specifier) = dep.get_type() {
          handle_specifier(
            graph,
            &mut seen,
            &mut pending,
            specifier,
            dep.is_dynamic,
          );
        }
      }
    }

    if let Some(dep) = maybe_types_dependency {
      handle_specifier(graph, &mut seen, &mut pending, &dep.specifier, false);
    }
  }

  result.maybe_hasher = maybe_hasher;

  result
}

fn resolve_npm_nv_ref(
  npm_resolver: &CliNpmResolver,
  node_resolver: &CliNodeResolver,
  nv_ref: &NpmPackageNvReference,
  referrer: &ModuleSpecifier,
) -> Option<ModuleSpecifier> {
  let pkg_dir = npm_resolver
    .as_managed()
    .unwrap()
    .resolve_pkg_folder_from_deno_module(nv_ref.nv())
    .ok()?;
  let resolved = node_resolver
    .resolve_package_subpath_from_deno_module(
      &pkg_dir,
      nv_ref.sub_path(),
      Some(referrer),
      node_resolver::ResolutionMode::Import,
      node_resolver::NodeResolutionKind::Types,
    )
    .ok()?;
  Some(resolved)
}

/// Matches the `@ts-check` pragma.
static TS_CHECK_RE: Lazy<Regex> =
  lazy_regex::lazy_regex!(r#"(?i)^\s*@ts-check(?:\s+|$)"#);

fn has_ts_check(media_type: MediaType, file_text: &str) -> bool {
  match &media_type {
    MediaType::JavaScript
    | MediaType::Mjs
    | MediaType::Cjs
    | MediaType::Jsx => get_leading_comments(file_text)
      .iter()
      .any(|text| TS_CHECK_RE.is_match(text)),
    MediaType::TypeScript
    | MediaType::Mts
    | MediaType::Cts
    | MediaType::Dts
    | MediaType::Dcts
    | MediaType::Dmts
    | MediaType::Tsx
    | MediaType::Json
    | MediaType::Wasm
    | MediaType::Css
    | MediaType::SourceMap
    | MediaType::Unknown => false,
  }
}

fn get_leading_comments(file_text: &str) -> Vec<String> {
  let mut chars = file_text.chars().peekable();

  // skip over the shebang
  if file_text.starts_with("#!") {
    // skip until the end of the line
    for c in chars.by_ref() {
      if c == '\n' {
        break;
      }
    }
  }

  let mut results = Vec::new();
  // now handle the comments
  while chars.peek().is_some() {
    // skip over any whitespace
    while chars
      .peek()
      .map(|c| char::is_whitespace(*c))
      .unwrap_or(false)
    {
      chars.next();
    }

    if chars.next() != Some('/') {
      break;
    }
    match chars.next() {
      Some('/') => {
        let mut text = String::new();
        for c in chars.by_ref() {
          if c == '\n' {
            break;
          } else {
            text.push(c);
          }
        }
        results.push(text);
      }
      Some('*') => {
        let mut text = String::new();
        while let Some(c) = chars.next() {
          if c == '*' && chars.peek() == Some(&'/') {
            chars.next();
            break;
          } else {
            text.push(c);
          }
        }
        results.push(text);
      }
      _ => break,
    }
  }
  results
}

#[cfg(test)]
mod test {
  use deno_ast::MediaType;

  use super::get_leading_comments;
  use super::has_ts_check;

  #[test]
  fn get_leading_comments_test() {
    assert_eq!(
      get_leading_comments(
        "#!/usr/bin/env deno\r\n// test\n/* 1 *//*2*///3\n//\n /**/  /*4 */"
      ),
      vec![
        " test".to_string(),
        " 1 ".to_string(),
        "2".to_string(),
        "3".to_string(),
        "".to_string(),
        "".to_string(),
        "4 ".to_string(),
      ]
    );
    assert_eq!(
      get_leading_comments("//1 /* */ \na;"),
      vec!["1 /* */ ".to_string(),]
    );
    assert_eq!(get_leading_comments("//"), vec!["".to_string()]);
  }

  #[test]
  fn has_ts_check_test() {
    assert!(has_ts_check(
      MediaType::JavaScript,
      "// @ts-check\nconsole.log(5);"
    ));
    assert!(has_ts_check(
      MediaType::JavaScript,
      "// deno-lint-ignore\n// @ts-check\n"
    ));
    assert!(!has_ts_check(
      MediaType::JavaScript,
      "test;\n// @ts-check\n"
    ));
    assert!(!has_ts_check(
      MediaType::JavaScript,
      "// ts-check\nconsole.log(5);"
    ));
  }
}
