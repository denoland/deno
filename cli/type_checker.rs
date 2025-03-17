// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::deno_json;
use deno_config::deno_json::CompilerOptionTypesDeserializeError;
use deno_config::workspace::WorkspaceDirectory;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_lib::util::hash::FastInsecureHasher;
use deno_semver::npm::NpmPackageNvReference;
use deno_terminal::colors;
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::args::deno_json::TsConfigResolver;
use crate::args::CliOptions;
use crate::args::DenoSubcommand;
use crate::args::TsConfig;
use crate::args::TsTypeLib;
use crate::args::TypeCheckMode;
use crate::cache::CacheDBHash;
use crate::cache::Caches;
use crate::cache::TypeCheckCache;
use crate::graph_util::maybe_additional_sloppy_imports_message;
use crate::graph_util::module_error_for_tsc_diagnostic;
use crate::graph_util::resolution_error_for_tsc_diagnostic;
use crate::graph_util::BuildFastCheckGraphOptions;
use crate::graph_util::ModuleGraphBuilder;
use crate::node::CliNodeResolver;
use crate::npm::installer::NpmInstaller;
use crate::npm::CliNpmResolver;
use crate::sys::CliSys;
use crate::tsc;
use crate::tsc::Diagnostics;
use crate::tsc::TypeCheckingCjsTracker;
use crate::util::path::to_percent_decoded_str;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
#[error("Type checking failed.{}", if self.can_skip {
  color_print::cstr!(
    "\n\n  <y>info:</y> The program failed type-checking, but it still might work correctly.\n  <c>hint:</c> Re-run with <u>--no-check</u> to skip type-checking.",
  )
} else {
  ""
})]
pub struct FailedTypeCheckingError {
  can_skip: bool,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CheckError {
  #[class(inherit)]
  #[error(transparent)]
  FailedTypeChecking(#[from] FailedTypeCheckingError),
  #[class(inherit)]
  #[error(transparent)]
  ToMaybeJsxImportSourceConfig(
    #[from] deno_config::workspace::ToMaybeJsxImportSourceConfigError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  TscExec(#[from] tsc::ExecError),
  #[class(inherit)]
  #[error(transparent)]
  CompilerOptionTypesDeserialize(#[from] CompilerOptionTypesDeserializeError),
  #[class(inherit)]
  #[error(transparent)]
  CompilerOptionsParse(#[from] deno_json::CompilerOptionsParseError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
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
  tsconfig_resolver: Arc<TsConfigResolver>,
  code_cache: Option<Arc<crate::cache::CodeCache>>,
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
    tsconfig_resolver: Arc<TsConfigResolver>,
    code_cache: Option<Arc<crate::cache::CodeCache>>,
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
      tsconfig_resolver,
      code_cache,
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
    let mut diagnostics = self.check_diagnostics(graph, options).await?;
    let mut failed = false;
    for result in diagnostics.by_ref() {
      let mut diagnostics = result?;
      diagnostics.emit_warnings();
      if diagnostics.has_diagnostic() {
        failed = true;
        log::error!("{}\n", diagnostics);
      }
    }
    if failed {
      Err(
        FailedTypeCheckingError {
          can_skip: !matches!(
            self.cli_options.sub_command(),
            DenoSubcommand::Check(_)
          ),
        }
        .into(),
      )
    } else {
      Ok(diagnostics.into_graph())
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
  ) -> Result<DiagnosticsByFolderIterator, CheckError> {
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
      return Ok(DiagnosticsByFolderIterator(
        DiagnosticsByFolderIteratorInner::Empty(Arc::new(graph)),
      ));
    }

    // node built-in specifiers use the @types/node package to determine
    // types, so inject that now (the caller should do this after the lockfile
    // has been written)
    if let Some(npm_installer) = &self.npm_installer {
      if graph.has_node_specifier {
        npm_installer.inject_synthetic_types_node_package().await?;
      }
    }

    log::debug!("Type checking");

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

    // split the roots by what we can send to the ts compiler all at once
    let grouped_roots =
      self.group_roots_by_compiler_options(&graph, options.lib)?;

    Ok(DiagnosticsByFolderIterator(
      DiagnosticsByFolderIteratorInner::Real(DiagnosticsByFolderRealIterator {
        graph,
        sys: &self.sys,
        cjs_tracker: &self.cjs_tracker,
        node_resolver: &self.node_resolver,
        npm_resolver: &self.npm_resolver,
        tsconfig_resolver: &self.tsconfig_resolver,
        log_level: self.cli_options.log_level(),
        npm_check_state_hash: check_state_hash(&self.npm_resolver),
        type_check_cache: TypeCheckCache::new(
          self.caches.type_checking_cache_db(),
        ),
        grouped_roots,
        options,
        seen_diagnotics: Default::default(),
        code_cache: self.code_cache.clone(),
      }),
    ))
  }

  /// Groups the roots based on the compiler options, which includes the
  /// resolved TsConfig and resolved compilerOptions.types
  fn group_roots_by_compiler_options<'a>(
    &'a self,
    graph: &ModuleGraph,
    lib: TsTypeLib,
  ) -> Result<IndexMap<CheckGroupKey<'a>, CheckGroupInfo>, CheckError> {
    let mut imports_for_specifier: HashMap<Arc<Url>, Rc<Vec<Url>>> =
      HashMap::with_capacity(self.tsconfig_resolver.folder_count());
    let mut roots_by_config: IndexMap<_, CheckGroupInfo> =
      IndexMap::with_capacity(self.tsconfig_resolver.folder_count());
    for root in &graph.roots {
      let folder = self.tsconfig_resolver.folder_for_specifier(root);
      let imports =
        match imports_for_specifier.entry(folder.dir.dir_url().clone()) {
          std::collections::hash_map::Entry::Occupied(entry) => {
            entry.get().clone()
          }
          std::collections::hash_map::Entry::Vacant(vacant_entry) => {
            let value = Rc::new(resolve_graph_imports_for_workspace_dir(
              graph,
              &folder.dir,
            ));
            vacant_entry.insert(value.clone());
            value
          }
        };
      let tsconfig = folder.lib_tsconfig(lib)?;
      let key = CheckGroupKey {
        ts_config: tsconfig,
        imports,
      };
      let entry = roots_by_config.entry(key);
      let entry = match entry {
        indexmap::map::Entry::Occupied(entry) => entry.into_mut(),
        indexmap::map::Entry::Vacant(entry) => entry.insert(CheckGroupInfo {
          roots: Default::default(),
          // this is slightly hacky. It's used as the referrer for resolving
          // npm imports in the key
          referrer: folder
            .dir
            .maybe_deno_json()
            .map(|d| d.specifier.clone())
            .unwrap_or_else(|| folder.dir.dir_url().as_ref().clone()),
        }),
      };
      entry.roots.push(root.clone());
    }
    Ok(roots_by_config)
  }
}

fn resolve_graph_imports_for_workspace_dir(
  graph: &ModuleGraph,
  dir: &WorkspaceDirectory,
) -> Vec<Url> {
  fn resolve_graph_imports_for_referrer<'a>(
    graph: &'a ModuleGraph,
    referrer: &'a Url,
  ) -> Option<impl Iterator<Item = Url> + 'a> {
    let imports = graph.imports.get(referrer)?;
    Some(
      imports
        .dependencies
        .values()
        .filter_map(|dep| dep.get_type().or_else(|| dep.get_code()))
        .map(|url| graph.resolve(url))
        .cloned(),
    )
  }

  let root_deno_json = dir.workspace.root_deno_json();
  let member_deno_json = dir.maybe_deno_json().filter(|c| {
    Some(&c.specifier) != root_deno_json.as_ref().map(|c| &c.specifier)
  });
  let mut specifiers = root_deno_json
    .map(|c| resolve_graph_imports_for_referrer(graph, &c.specifier))
    .into_iter()
    .flatten()
    .flatten()
    .chain(
      member_deno_json
        .map(|c| resolve_graph_imports_for_referrer(graph, &c.specifier))
        .into_iter()
        .flatten()
        .flatten(),
    )
    .collect::<Vec<_>>();
  specifiers.sort();
  specifiers
}

/// Key to use to group roots together by config.
#[derive(Debug, Hash, PartialEq, Eq)]
struct CheckGroupKey<'a> {
  ts_config: &'a Arc<TsConfig>,
  imports: Rc<Vec<Url>>,
}

struct CheckGroupInfo {
  roots: Vec<Url>,
  referrer: Url,
}

pub struct DiagnosticsByFolderIterator<'a>(
  DiagnosticsByFolderIteratorInner<'a>,
);

impl DiagnosticsByFolderIterator<'_> {
  pub fn into_graph(self) -> Arc<ModuleGraph> {
    match self.0 {
      DiagnosticsByFolderIteratorInner::Empty(module_graph) => module_graph,
      DiagnosticsByFolderIteratorInner::Real(r) => r.graph,
    }
  }
}

impl Iterator for DiagnosticsByFolderIterator<'_> {
  type Item = Result<Diagnostics, CheckError>;

  fn next(&mut self) -> Option<Self::Item> {
    match &mut self.0 {
      DiagnosticsByFolderIteratorInner::Empty(_) => None,
      DiagnosticsByFolderIteratorInner::Real(r) => r.next(),
    }
  }
}

enum DiagnosticsByFolderIteratorInner<'a> {
  Empty(Arc<ModuleGraph>),
  Real(DiagnosticsByFolderRealIterator<'a>),
}

struct DiagnosticsByFolderRealIterator<'a> {
  graph: Arc<ModuleGraph>,
  sys: &'a CliSys,
  cjs_tracker: &'a Arc<TypeCheckingCjsTracker>,
  node_resolver: &'a Arc<CliNodeResolver>,
  npm_resolver: &'a CliNpmResolver,
  tsconfig_resolver: &'a TsConfigResolver,
  type_check_cache: TypeCheckCache,
  grouped_roots: IndexMap<CheckGroupKey<'a>, CheckGroupInfo>,
  log_level: Option<log::Level>,
  npm_check_state_hash: Option<u64>,
  seen_diagnotics: HashSet<String>,
  options: CheckOptions,
  code_cache: Option<Arc<crate::cache::CodeCache>>,
}

impl Iterator for DiagnosticsByFolderRealIterator<'_> {
  type Item = Result<Diagnostics, CheckError>;

  fn next(&mut self) -> Option<Self::Item> {
    let (group_key, group_info) = self.grouped_roots.shift_remove_index(0)?;
    let mut result = self.check_diagnostics_in_folder(&group_key, group_info);
    if let Ok(diagnostics) = &mut result {
      diagnostics.retain(|d| {
        if let (Some(file_name), Some(start)) = (&d.file_name, &d.start) {
          let data = format!(
            "{}{}:{}:{}{}",
            d.code,
            file_name,
            start.line,
            start.character,
            d.message_text.as_deref().unwrap_or_default()
          );
          self.seen_diagnotics.insert(data)
        } else {
          // show these for each type of config
          true
        }
      });
    }
    Some(result)
  }
}

impl<'a> DiagnosticsByFolderRealIterator<'a> {
  #[allow(clippy::too_many_arguments)]
  fn check_diagnostics_in_folder(
    &self,
    group_key: &'a CheckGroupKey<'a>,
    group_info: CheckGroupInfo,
  ) -> Result<Diagnostics, CheckError> {
    fn log_provided_roots(provided_roots: &[Url]) {
      for root in provided_roots {
        log::info!(
          "{} {}",
          colors::green("Check"),
          to_percent_decoded_str(root.as_str())
        );
      }
    }

    // walk the graph
    let ts_config = group_key.ts_config;
    let mut graph_walker = GraphWalker::new(
      &self.graph,
      self.sys,
      self.node_resolver,
      self.npm_resolver,
      self.tsconfig_resolver,
      self.npm_check_state_hash,
      ts_config.as_ref(),
      self.options.type_check_mode,
    );
    let mut provided_roots = group_info.roots;
    for import in group_key.imports.iter() {
      graph_walker.add_config_import(import, &group_info.referrer);
    }

    for root in &provided_roots {
      graph_walker.add_root(root);
    }

    let TscRoots {
      roots: root_names,
      missing_diagnostics,
      maybe_check_hash,
    } = graph_walker.into_tsc_roots();

    let mut missing_diagnostics = missing_diagnostics.filter(|d| {
      self.should_include_diagnostic(self.options.type_check_mode, d)
    });
    missing_diagnostics.apply_fast_check_source_maps(&self.graph);

    if root_names.is_empty() {
      if missing_diagnostics.has_diagnostic() {
        log_provided_roots(&provided_roots);
      }
      return Ok(missing_diagnostics);
    }

    if !self.options.reload && !missing_diagnostics.has_diagnostic() {
      // do not type check if we know this is type checked
      if let Some(check_hash) = maybe_check_hash {
        if self.type_check_cache.has_check_hash(check_hash) {
          log::debug!("Already type checked {}", group_info.referrer);
          return Ok(Default::default());
        }
      }
    }

    // log out the roots that we're checking
    log_provided_roots(&provided_roots);

    // the first root will always either be the specifier that the user provided
    // or the first specifier in a directory
    let first_root = provided_roots.remove(0);

    // while there might be multiple roots, we can't "merge" the build info, so we
    // try to retrieve the build info for first root, which is the most common use
    // case.
    let maybe_tsbuildinfo = if self.options.reload {
      None
    } else {
      self.type_check_cache.get_tsbuildinfo(&first_root)
    };
    // to make tsc build info work, we need to consistently hash modules, so that
    // tsc can better determine if an emit is still valid or not, so we provide
    // that data here.
    let tsconfig_hash_data = FastInsecureHasher::new_deno_versioned()
      .write_hashable(ts_config)
      .finish();
    let code_cache = self.code_cache.as_ref().map(|c| {
      let c: Arc<dyn deno_runtime::code_cache::CodeCache> = c.clone();
      c
    });
    let response = tsc::exec(
      tsc::Request {
        config: ts_config.clone(),
        debug: self.log_level == Some(log::Level::Debug),
        graph: self.graph.clone(),
        hash_data: tsconfig_hash_data,
        maybe_npm: Some(tsc::RequestNpmState {
          cjs_tracker: self.cjs_tracker.clone(),
          node_resolver: self.node_resolver.clone(),
          npm_resolver: self.npm_resolver.clone(),
        }),
        maybe_tsbuildinfo,
        root_names,
        check_mode: self.options.type_check_mode,
      },
      code_cache,
    )?;

    let mut response_diagnostics = response.diagnostics.filter(|d| {
      self.should_include_diagnostic(self.options.type_check_mode, d)
    });
    response_diagnostics.apply_fast_check_source_maps(&self.graph);
    let mut diagnostics = missing_diagnostics;
    diagnostics.extend(response_diagnostics);

    if let Some(tsbuildinfo) = response.maybe_tsbuildinfo {
      self
        .type_check_cache
        .set_tsbuildinfo(&first_root, &tsbuildinfo);
    }

    if !diagnostics.has_diagnostic() {
      if let Some(check_hash) = maybe_check_hash {
        self.type_check_cache.add_check_hash(check_hash);
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
    // this shouldn't check for duplicate diagnostics across folders because
    // we don't want to accidentally mark a folder as being successful and save
    // to the check cache if a previous folder caused a diagnostic
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

struct TscRoots {
  roots: Vec<(ModuleSpecifier, MediaType)>,
  missing_diagnostics: tsc::Diagnostics,
  maybe_check_hash: Option<CacheDBHash>,
}

struct GraphWalker<'a> {
  graph: &'a ModuleGraph,
  sys: &'a CliSys,
  node_resolver: &'a CliNodeResolver,
  npm_resolver: &'a CliNpmResolver,
  tsconfig_resolver: &'a TsConfigResolver,
  maybe_hasher: Option<FastInsecureHasher>,
  seen: HashSet<&'a Url>,
  pending: VecDeque<(&'a Url, bool)>,
  has_seen_node_builtin: bool,
  roots: Vec<(ModuleSpecifier, MediaType)>,
  missing_diagnostics: tsc::Diagnostics,
}

impl<'a> GraphWalker<'a> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    graph: &'a ModuleGraph,
    sys: &'a CliSys,
    node_resolver: &'a CliNodeResolver,
    npm_resolver: &'a CliNpmResolver,
    tsconfig_resolver: &'a TsConfigResolver,
    npm_cache_state_hash: Option<u64>,
    ts_config: &TsConfig,
    type_check_mode: TypeCheckMode,
  ) -> Self {
    let maybe_hasher = npm_cache_state_hash.map(|npm_cache_state_hash| {
      let mut hasher = FastInsecureHasher::new_deno_versioned();
      hasher.write_hashable(npm_cache_state_hash);
      hasher.write_u8(match type_check_mode {
        TypeCheckMode::All => 0,
        TypeCheckMode::Local => 1,
        TypeCheckMode::None => 2,
      });
      hasher.write_hashable(graph.has_node_specifier);
      hasher.write_hashable(ts_config);
      hasher
    });
    Self {
      graph,
      sys,
      node_resolver,
      npm_resolver,
      tsconfig_resolver,
      maybe_hasher,
      seen: HashSet::with_capacity(
        graph.imports.len() + graph.specifiers_count(),
      ),
      pending: VecDeque::new(),
      has_seen_node_builtin: false,
      roots: Vec::with_capacity(graph.imports.len() + graph.specifiers_count()),
      missing_diagnostics: Default::default(),
    }
  }

  pub fn add_config_import(&mut self, specifier: &'a Url, referrer: &Url) {
    let specifier = self.graph.resolve(specifier);
    if self.seen.insert(specifier) {
      if let Ok(nv_ref) = NpmPackageNvReference::from_specifier(specifier) {
        match self.resolve_npm_nv_ref(&nv_ref, referrer) {
          Some(resolved) => {
            let mt = MediaType::from_specifier(&resolved);
            self.roots.push((resolved, mt));
          }
          None => {
            self
              .missing_diagnostics
              .push(tsc::Diagnostic::from_missing_error(
                specifier.as_str(),
                None,
                maybe_additional_sloppy_imports_message(self.sys, specifier),
              ));
          }
        }
      } else {
        self.pending.push_back((specifier, false));
        self.resolve_pending();
      }
    }
  }

  pub fn add_root(&mut self, root: &'a Url) {
    let specifier = self.graph.resolve(root);
    if self.seen.insert(specifier) {
      self.pending.push_back((specifier, false));
    }

    self.resolve_pending()
  }

  /// Transform the graph into root specifiers that we can feed `tsc`. We have to
  /// provide the media type for root modules because `tsc` does not "resolve" the
  /// media type like other modules, as well as a root specifier needs any
  /// redirects resolved. We need to include all the emittable files in
  /// the roots, so they get type checked and optionally emitted,
  /// otherwise they would be ignored if only imported into JavaScript.
  pub fn into_tsc_roots(self) -> TscRoots {
    TscRoots {
      roots: self.roots,
      missing_diagnostics: self.missing_diagnostics,
      maybe_check_hash: self.maybe_hasher.map(|h| CacheDBHash::new(h.finish())),
    }
  }

  fn resolve_pending(&mut self) {
    while let Some((specifier, is_dynamic)) = self.pending.pop_front() {
      let module = match self.graph.try_get(specifier) {
        Ok(Some(module)) => module,
        Ok(None) => continue,
        Err(err) => {
          if !is_dynamic {
            if let Some(err) = module_error_for_tsc_diagnostic(self.sys, err) {
              self.missing_diagnostics.push(
                tsc::Diagnostic::from_missing_error(
                  err.specifier.as_str(),
                  err.maybe_range,
                  maybe_additional_sloppy_imports_message(
                    self.sys,
                    err.specifier,
                  ),
                ),
              );
            }
          }
          continue;
        }
      };
      if is_dynamic && !self.seen.insert(specifier) {
        continue;
      }
      if let Some(entry) = self.maybe_get_check_entry(module) {
        self.roots.push(entry);
      }

      let mut maybe_module_dependencies = None;
      let mut maybe_types_dependency = None;
      match module {
        Module::Js(module) => {
          maybe_module_dependencies =
            Some(module.dependencies_prefer_fast_check());
          maybe_types_dependency = module
            .maybe_types_dependency
            .as_ref()
            .and_then(|d| d.dependency.ok());
        }
        Module::Wasm(module) => {
          maybe_module_dependencies = Some(&module.dependencies);
        }
        Module::Json(_) | Module::Npm(_) | Module::External(_) => {}
        Module::Node(_) => {
          if !self.has_seen_node_builtin {
            self.has_seen_node_builtin = true;
            // inject a specifier that will resolve node types
            self.roots.push((
              ModuleSpecifier::parse("asset:///node_types.d.ts").unwrap(),
              MediaType::Dts,
            ));
          }
        }
      }

      if let Some(deps) = maybe_module_dependencies {
        for dep in deps.values() {
          // walk both the code and type dependencies
          for resolution in [&dep.maybe_type, &dep.maybe_code] {
            match resolution {
              deno_graph::Resolution::Ok(resolution) => {
                self.handle_specifier(&resolution.specifier, dep.is_dynamic);
              }
              deno_graph::Resolution::Err(resolution_error) => {
                if let Some(err) =
                  resolution_error_for_tsc_diagnostic(resolution_error)
                {
                  self.missing_diagnostics.push(
                    tsc::Diagnostic::from_missing_error(
                      err.specifier,
                      err.maybe_range,
                      None,
                    ),
                  );
                }
              }
              deno_graph::Resolution::None => {}
            }
          }
        }
      }

      if let Some(dep) = maybe_types_dependency {
        self.handle_specifier(&dep.specifier, false);
      }
    }
  }

  fn maybe_get_check_entry(
    &mut self,
    module: &deno_graph::Module,
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
            if self
              .tsconfig_resolver
              .check_js_for_specifier(&module.specifier)
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
          | MediaType::Html
          | MediaType::SourceMap
          | MediaType::Sql
          | MediaType::Unknown => None,
        };
        if result.is_some() {
          if let Some(hasher) = &mut self.maybe_hasher {
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
        if let Some(hasher) = &mut self.maybe_hasher {
          hasher.write_str(module.specifier.as_str());
          hasher.write_str(&module.source);
        }
        None
      }
      Module::Wasm(module) => {
        if let Some(hasher) = &mut self.maybe_hasher {
          hasher.write_str(module.specifier.as_str());
          hasher.write_str(&module.source_dts);
        }
        Some((module.specifier.clone(), MediaType::Dmts))
      }
      Module::External(module) => {
        if let Some(hasher) = &mut self.maybe_hasher {
          hasher.write_str(module.specifier.as_str());
        }

        None
      }
    }
  }

  fn handle_specifier(
    &mut self,
    specifier: &'a ModuleSpecifier,
    is_dynamic: bool,
  ) {
    let specifier = self.graph.resolve(specifier);
    if is_dynamic {
      if !self.seen.contains(specifier) {
        self.pending.push_back((specifier, true));
      }
    } else if self.seen.insert(specifier) {
      self.pending.push_back((specifier, false));
    }
  }

  fn resolve_npm_nv_ref(
    &self,
    nv_ref: &NpmPackageNvReference,
    referrer: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let pkg_dir = self
      .npm_resolver
      .as_managed()
      .unwrap()
      .resolve_pkg_folder_from_deno_module(nv_ref.nv())
      .ok()?;
    let resolved = self
      .node_resolver
      .resolve_package_subpath_from_deno_module(
        &pkg_dir,
        nv_ref.sub_path(),
        Some(referrer),
        node_resolver::ResolutionMode::Import,
        node_resolver::NodeResolutionKind::Types,
      )
      .ok()?;
    resolved.into_url().ok()
  }
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
    | MediaType::Html
    | MediaType::SourceMap
    | MediaType::Sql
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
