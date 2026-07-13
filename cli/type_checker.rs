// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::deno_json::CompilerOptionTypesDeserializeError;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::ResolutionError;
use deno_lib::util::hash::FastInsecureHasher;
use deno_resolver::deno_json::CompilerOptionsData;
use deno_resolver::deno_json::CompilerOptionsParseError;
use deno_resolver::deno_json::CompilerOptionsResolver;
use deno_resolver::deno_json::JsxImportSourceConfigResolver;
use deno_resolver::deno_json::ToMaybeJsxImportSourceConfigError;
use deno_resolver::graph::maybe_additional_sloppy_imports_message;
use deno_semver::npm::NpmPackageReqReference;
use deno_terminal::colors;
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::args::CliOptions;
use crate::args::CompilerOptions;
use crate::args::DenoSubcommand;
use crate::args::TsTypeLib;
use crate::args::TypeCheckMode;
use crate::cache::CacheDBHash;
use crate::cache::Caches;
use crate::cache::TypeCheckCache;
use crate::graph_util::BuildFastCheckGraphOptions;
use crate::graph_util::ModuleGraphBuilder;
use crate::graph_util::module_error_for_tsc_diagnostic;
use crate::node::CliNodeResolver;
use crate::node::CliPackageJsonResolver;
use crate::npm::CliNpmResolver;
use crate::sys::CliSys;
use crate::tsc;
use crate::tsc::Diagnostics;
use crate::tsc::TypeCheckingCjsTracker;

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

#[derive(Debug, boxed_error::Boxed, deno_error::JsError)]
pub struct CheckError(pub Box<CheckErrorKind>);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CheckErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  FailedTypeChecking(#[from] FailedTypeCheckingError),
  #[class(inherit)]
  #[error(transparent)]
  ToMaybeJsxImportSourceConfig(#[from] ToMaybeJsxImportSourceConfigError),
  #[class(inherit)]
  #[error(transparent)]
  TscExec(#[from] tsc::ExecError),
  #[class(inherit)]
  #[error(transparent)]
  CompilerOptionTypesDeserialize(#[from] CompilerOptionTypesDeserializeError),
  #[class(inherit)]
  #[error(transparent)]
  CompilerOptionsParse(#[from] CompilerOptionsParseError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

/// Result of emitting declaration files via [`TypeChecker::emit_declarations`].
pub struct EmitDeclarationsResult {
  pub diagnostics: Diagnostics,
  /// Emitted `.d.ts` files keyed by their specifier (e.g. `file:///path/to/file.d.ts`).
  pub emitted_files: BTreeMap<String, String>,
}

/// Options for performing a check of a module graph. Note that the decision to
/// emit or not is determined by the `compiler_options` settings.
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
  node_resolver: Arc<CliNodeResolver>,
  npm_resolver: CliNpmResolver,
  package_json_resolver: Arc<CliPackageJsonResolver>,
  sys: CliSys,
  compiler_options_resolver: Arc<CompilerOptionsResolver>,
  code_cache: Option<Arc<crate::cache::CodeCache>>,
}

impl TypeChecker {
  #[allow(clippy::too_many_arguments, reason = "construction")]
  pub fn new(
    caches: Arc<Caches>,
    cjs_tracker: Arc<TypeCheckingCjsTracker>,
    cli_options: Arc<CliOptions>,
    module_graph_builder: Arc<ModuleGraphBuilder>,
    node_resolver: Arc<CliNodeResolver>,
    npm_resolver: CliNpmResolver,
    package_json_resolver: Arc<CliPackageJsonResolver>,
    sys: CliSys,
    compiler_options_resolver: Arc<CompilerOptionsResolver>,
    code_cache: Option<Arc<crate::cache::CodeCache>>,
  ) -> Self {
    Self {
      caches,
      cjs_tracker,
      cli_options,
      module_graph_builder,
      node_resolver,
      npm_resolver,
      package_json_resolver,
      sys,
      compiler_options_resolver,
      code_cache,
    }
  }

  pub fn create_request_npm_state(&self) -> tsc::RequestNpmState {
    tsc::RequestNpmState {
      cjs_tracker: self.cjs_tracker.clone(),
      node_resolver: self.node_resolver.clone(),
      npm_resolver: self.npm_resolver.clone(),
    }
  }

  /// Type-check and emit `.d.ts` declaration files for the given module graph.
  ///
  /// This runs the TypeScript compiler with `emitDeclarationOnly: true` and
  /// returns the emitted `.d.ts` file contents keyed by their specifier paths.
  pub fn emit_declarations(
    &self,
    graph: Arc<ModuleGraph>,
    root_names: Vec<(ModuleSpecifier, MediaType)>,
    lib: TsTypeLib,
  ) -> Result<EmitDeclarationsResult, CheckError> {
    let first_specifier = &root_names[0].0;
    let compiler_options_data = self
      .compiler_options_resolver
      .for_specifier(first_specifier);
    let base_compiler_options =
      compiler_options_data.compiler_options_for_lib(lib)?;

    // Merge declaration-specific options into the base compiler options
    let mut config_value =
      deno_core::serde_json::to_value(base_compiler_options.as_ref())
        .map_err(|e| CheckErrorKind::Other(JsErrorBox::from_err(e)))?;
    if let Some(config_obj) = config_value.as_object_mut() {
      config_obj.insert(
        "declaration".into(),
        deno_core::serde_json::Value::Bool(true),
      );
      config_obj.insert(
        "emitDeclarationOnly".into(),
        deno_core::serde_json::Value::Bool(true),
      );
      config_obj
        .insert("noEmit".into(), deno_core::serde_json::Value::Bool(false));
    }

    let compiler_options = Arc::new(CompilerOptions::new(config_value));

    let hash_data = FastInsecureHasher::new_deno_versioned()
      .write_hashable(&compiler_options)
      .finish();

    let jsx_import_source_config_resolver = Arc::new(
      JsxImportSourceConfigResolver::from_compiler_options_resolver(
        &self.compiler_options_resolver,
      )?,
    );

    let response = tsc::exec(
      tsc::Request {
        config: compiler_options,
        debug: self.cli_options.log_level() == Some(log::Level::Debug),
        graph,
        jsx_import_source_config_resolver,
        hash_data,
        maybe_npm: Some(self.create_request_npm_state()),
        maybe_tsbuildinfo: None,
        root_names,
        // Declaration emit requires full type-checking regardless of
        // the user's type_check_mode setting.
        check_mode: TypeCheckMode::All,
        initial_cwd: self.cli_options.initial_cwd().to_path_buf(),
        capture_emitted_files: true,
      },
      None,
    )?;

    Ok(EmitDeclarationsResult {
      diagnostics: response.diagnostics,
      emitted_files: response.emitted_files,
    })
  }

  /// Type check the module graph.
  ///
  /// It is expected that it is determined if a check and/or emit is validated
  /// before the function is called.
  pub fn check(
    &self,
    graph: ModuleGraph,
    options: CheckOptions,
  ) -> Result<Arc<ModuleGraph>, CheckError> {
    let mut diagnostics_iter = self.check_diagnostics(graph, options)?;
    // Drain the iterator first so that all the "Check ..." lines (which are
    // printed while type checking each folder) are emitted before any
    // diagnostics. Otherwise, in a workspace with multiple folders, errors
    // from one folder would be printed in the middle of the "Check ..." lines
    // of the following folders.
    let mut all_diagnostics = Vec::with_capacity(diagnostics_iter.remaining());
    for result in diagnostics_iter.by_ref() {
      all_diagnostics.push(result?);
    }
    let mut failed = false;
    for mut diagnostics in all_diagnostics {
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
      Ok(diagnostics_iter.into_graph())
    }
  }

  /// Type check the module graph returning its diagnostics.
  ///
  /// It is expected that it is determined if a check and/or emit is validated
  /// before the function is called.
  /// Walk the module graph exactly as the in-process checker does — for the
  /// native (external tsc) check path in `crate::tools::check`. Returns deno's
  /// own graph diagnostics (missing modules + hints) plus a combined check hash
  /// over every compiler-options group, with the pinned tsc version folded in.
  /// Native check runs a single external tsc over the whole generated
  /// `tsconfig.json`, so the per-group hashes are combined into one: a hit means
  /// nothing the compiler sees has changed, and the spawn can be skipped.
  pub(crate) fn walk_graph_for_native_check(
    &self,
    graph: &ModuleGraph,
    lib: TsTypeLib,
    type_check_mode: TypeCheckMode,
  ) -> Result<(tsc::Diagnostics, Option<CacheDBHash>), CheckError> {
    // Packages importable by bare specifier (workspace members + `links`),
    // used to enhance import errors. TODO: also chain `links` packages to match
    // `check_diagnostics` exactly.
    let bare_importable_pkg_names: Vec<String> = self
      .cli_options
      .workspace()
      .resolver_jsr_pkgs()
      .map(|pkg| pkg.name)
      .collect();
    let npm_check_state_hash = check_state_hash(&self.npm_resolver);
    let groups = self.group_roots_by_compiler_options(graph, lib)?;
    let mut missing_diagnostics = tsc::Diagnostics::default();
    let mut combined = FastInsecureHasher::new_deno_versioned();
    combined.write_hashable(crate::tsc::native::TYPESCRIPT_VERSION);
    let mut any_hash = false;
    for group in groups {
      let mut walker = GraphWalker::new(
        graph,
        &self.sys,
        &self.node_resolver,
        &self.npm_resolver,
        &self.compiler_options_resolver,
        &bare_importable_pkg_names,
        npm_check_state_hash,
        group.compiler_options.as_ref(),
        type_check_mode,
      );
      for import in group.imports.iter() {
        walker.add_config_import(import, &group.referrer);
      }
      for root in &group.roots {
        walker.add_root(root);
      }
      let tsc_roots = walker.into_tsc_roots();
      missing_diagnostics.extend(tsc_roots.missing_diagnostics);
      if let Some(hash) = tsc_roots.maybe_check_hash {
        combined.write_hashable(hash);
        any_hash = true;
      }
    }
    let maybe_check_hash =
      any_hash.then(|| CacheDBHash::new(combined.finish()));
    Ok((missing_diagnostics, maybe_check_hash))
  }

  /// The type-check cache, used by the native check path to skip re-running the
  /// external compiler when the graph hash is unchanged.
  pub(crate) fn type_check_cache(&self) -> TypeCheckCache {
    TypeCheckCache::new(self.caches.type_checking_cache_db())
  }

  pub fn check_diagnostics(
    &self,
    mut graph: ModuleGraph,
    options: CheckOptions,
  ) -> Result<DiagnosticsByFolderIterator<'_>, CheckError> {
    if !options.type_check_mode.is_true() || graph.roots.is_empty() {
      return Ok(DiagnosticsByFolderIterator(
        DiagnosticsByFolderIteratorInner::Empty(Arc::new(graph)),
      ));
    }

    log::debug!("Type checking");

    // add fast check to the graph before getting the roots
    if options.build_fast_check_graph {
      self.module_graph_builder.build_fast_check_graph(
        &mut graph,
        BuildFastCheckGraphOptions {
          workspace_fast_check: deno_graph::WorkspaceFastCheckOption::Disabled,
          fast_check_dts: false,
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
        jsx_import_source_config_resolver: Arc::new(
          JsxImportSourceConfigResolver::from_compiler_options_resolver(
            &self.compiler_options_resolver,
          )?,
        ),
        node_resolver: &self.node_resolver,
        npm_resolver: &self.npm_resolver,
        _package_json_resolver: &self.package_json_resolver,
        compiler_options_resolver: &self.compiler_options_resolver,
        log_level: self.cli_options.log_level(),
        npm_check_state_hash: check_state_hash(&self.npm_resolver),
        type_check_cache: TypeCheckCache::new(
          self.caches.type_checking_cache_db(),
        ),
        groups: grouped_roots,
        current_group_index: 0,
        options,
        seen_diagnotics: Default::default(),
        code_cache: self.code_cache.clone(),
        initial_cwd: self.cli_options.initial_cwd().to_path_buf(),
        current_dir: deno_path_util::url_from_directory_path(
          self.cli_options.initial_cwd(),
        )
        .map_err(|e| CheckErrorKind::Other(JsErrorBox::from_err(e)))?,
        bare_importable_pkg_names: self
          .cli_options
          .workspace()
          .resolver_jsr_pkgs()
          .map(|pkg| pkg.name)
          .collect(),
      }),
    ))
  }

  /// Groups the roots based on the compiler options, which includes the
  /// resolved CompilerOptions and resolved compilerOptions.types
  fn group_roots_by_compiler_options<'a>(
    &'a self,
    graph: &ModuleGraph,
    lib: TsTypeLib,
  ) -> Result<Vec<CheckGroup<'a>>, CheckError> {
    let group_count = self.compiler_options_resolver.size();
    let mut imports_for_specifier = HashMap::with_capacity(group_count);
    let mut groups_by_key = IndexMap::with_capacity(group_count);
    for root in &graph.roots {
      let compiler_options_data =
        self.compiler_options_resolver.for_specifier(root);
      let compiler_options =
        compiler_options_data.compiler_options_for_lib(lib)?;
      let imports = imports_for_specifier
        .entry(compiler_options_data.sources.last().map(|s| &s.specifier))
        .or_insert_with(|| {
          Rc::new(resolve_graph_imports_for_compiler_options_data(
            graph,
            compiler_options_data,
          ))
        })
        .clone();
      let group_key = (compiler_options, imports.clone());
      let group = groups_by_key.entry(group_key).or_insert_with(|| {
        let dir = self.cli_options.workspace().resolve_member_dir(root);
        CheckGroup {
          roots: Default::default(),
          compiler_options,
          imports,
          // this is slightly hacky. It's used as the referrer for resolving
          // npm imports in the key
          referrer: dir
            .member_or_root_deno_json()
            .map(|d| d.specifier.clone())
            .unwrap_or_else(|| dir.dir_url().as_ref().clone()),
        }
      });
      group.roots.push(root.clone());
    }
    Ok(groups_by_key.into_values().collect())
  }
}

/// This function assumes that 'graph imports' strictly refer to tsconfig
/// `files` and `compilerOptions.types` which they currently do. In fact, if
/// they were more general than that, we don't really have sufficient context to
/// group them for type-checking.
fn resolve_graph_imports_for_compiler_options_data(
  graph: &ModuleGraph,
  compiler_options: &CompilerOptionsData,
) -> Vec<Url> {
  let mut specifiers = compiler_options
    .sources
    .iter()
    .map(|s| &s.specifier)
    .filter_map(|s| graph.imports.get(s.as_ref()))
    .flat_map(|i| i.dependencies.values())
    .filter_map(|d| Some(graph.resolve(d.get_type().or_else(|| d.get_code())?)))
    .cloned()
    .collect::<Vec<_>>();
  specifiers.sort();
  specifiers
}

#[derive(Debug)]
struct CheckGroup<'a> {
  roots: Vec<Url>,
  imports: Rc<Vec<Url>>,
  referrer: Url,
  compiler_options: &'a Arc<CompilerOptions>,
}

pub struct DiagnosticsByFolderIterator<'a>(
  DiagnosticsByFolderIteratorInner<'a>,
);

impl DiagnosticsByFolderIterator<'_> {
  /// Number of folders remaining to be checked, i.e. the exact number of items
  /// this iterator will still yield.
  pub fn remaining(&self) -> usize {
    match &self.0 {
      DiagnosticsByFolderIteratorInner::Empty(_) => 0,
      DiagnosticsByFolderIteratorInner::Real(r) => {
        r.groups.len().saturating_sub(r.current_group_index)
      }
    }
  }

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

#[allow(
  clippy::large_enum_variant,
  reason = "large variant is used more often"
)]
enum DiagnosticsByFolderIteratorInner<'a> {
  Empty(Arc<ModuleGraph>),
  Real(DiagnosticsByFolderRealIterator<'a>),
}

struct DiagnosticsByFolderRealIterator<'a> {
  graph: Arc<ModuleGraph>,
  sys: &'a CliSys,
  cjs_tracker: &'a Arc<TypeCheckingCjsTracker>,
  jsx_import_source_config_resolver: Arc<JsxImportSourceConfigResolver>,
  node_resolver: &'a Arc<CliNodeResolver>,
  npm_resolver: &'a CliNpmResolver,
  _package_json_resolver: &'a Arc<CliPackageJsonResolver>,
  compiler_options_resolver: &'a CompilerOptionsResolver,
  type_check_cache: TypeCheckCache,
  groups: Vec<CheckGroup<'a>>,
  current_group_index: usize,
  log_level: Option<log::Level>,
  npm_check_state_hash: Option<u64>,
  seen_diagnotics: HashSet<String>,
  options: CheckOptions,
  code_cache: Option<Arc<crate::cache::CodeCache>>,
  initial_cwd: PathBuf,
  current_dir: Url,
  /// Names of packages importable by bare specifier (workspace members and
  /// packages linked via the "links" field), used to enhance import errors.
  bare_importable_pkg_names: Vec<String>,
}

impl Iterator for DiagnosticsByFolderRealIterator<'_> {
  type Item = Result<Diagnostics, CheckError>;

  fn next(&mut self) -> Option<Self::Item> {
    let check_group = self.groups.get(self.current_group_index)?;
    self.current_group_index += 1;
    let mut result = self.check_diagnostics_in_folder(check_group);
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

/// Converts the list of ambient module names to regex string
/// Hash of the npm resolution state, folded into the type-check cache key so
/// the cache busts when npm deps (or nodeModulesDir) change. Shared by the
/// in-process checker and the native (external tsc) check path.
fn check_state_hash(resolver: &CliNpmResolver) -> Option<u64> {
  match resolver {
    // not feasible and probably slower to compute
    CliNpmResolver::Byonm(_) => None,
    CliNpmResolver::Managed(resolver) => {
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

pub fn ambient_modules_to_regex_string(ambient_modules: &[String]) -> String {
  let mut regex_string = String::with_capacity(ambient_modules.len() * 8);
  regex_string.push_str("^(");
  let last = ambient_modules.len() - 1;
  for (idx, part) in ambient_modules.iter().enumerate() {
    let trimmed = part.trim_matches('"');
    let escaped = regex::escape(trimmed);
    let regex = escaped.replace("\\*", ".*");
    regex_string.push_str(&regex);
    if idx != last {
      regex_string.push('|');
    }
  }
  regex_string.push_str(")$");
  regex_string
}

impl DiagnosticsByFolderRealIterator<'_> {
  fn check_diagnostics_in_folder(
    &self,
    check_group: &CheckGroup,
  ) -> Result<Diagnostics, CheckError> {
    fn log_provided_roots(provided_roots: &[Url], current_dir: &Url) {
      for root in provided_roots {
        log::info!(
          "{} {}",
          colors::green("Check"),
          crate::util::path::relative_specifier_path_for_display(
            current_dir,
            root
          ),
        );
      }
    }

    // walk the graph
    let mut graph_walker = GraphWalker::new(
      &self.graph,
      self.sys,
      self.node_resolver,
      self.npm_resolver,
      self.compiler_options_resolver,
      &self.bare_importable_pkg_names,
      self.npm_check_state_hash,
      check_group.compiler_options,
      self.options.type_check_mode,
    );
    for import in check_group.imports.iter() {
      graph_walker.add_config_import(import, &check_group.referrer);
    }

    for root in &check_group.roots {
      graph_walker.add_root(root);
    }

    // Add JSX runtime types to the roots so that TS can resolve
    // the jsx-runtime module during type checking. Without this,
    // TS 6.0+ emits TS2875 because it validates that the JSX
    // runtime module actually exports the JSX namespace.
    self.add_jsx_runtime_types(&mut graph_walker, check_group);

    let TscRoots {
      roots: root_names,
      missing_diagnostics,
      used_ts_expect_error_directives,
      maybe_check_hash,
    } = graph_walker.into_tsc_roots();

    let mut missing_diagnostics = missing_diagnostics.filter(|d| {
      self.should_include_diagnostic(self.options.type_check_mode, d)
        && !self.is_untagged_jsdoc_dynamic_import_diagnostic(d)
    });
    missing_diagnostics.apply_fast_check_source_maps(&self.graph);

    if root_names.is_empty() {
      if missing_diagnostics.has_diagnostic() {
        log_provided_roots(&check_group.roots, &self.current_dir);
      }
      return Ok(missing_diagnostics);
    }

    if !self.options.reload && !missing_diagnostics.has_diagnostic() {
      // do not type check if we know this is type checked
      if let Some(check_hash) = maybe_check_hash
        && self.type_check_cache.has_check_hash(check_hash)
      {
        log::debug!("Already type checked {}", &check_group.referrer);
        return Ok(Default::default());
      }
    }

    // log out the roots that we're checking
    log_provided_roots(&check_group.roots, &self.current_dir);

    // the first root will always either be the specifier that the user provided
    // or the first specifier in a directory
    let first_root = check_group
      .roots
      .first()
      .expect("must be at least one root");

    // while there might be multiple roots, we can't "merge" the build info, so we
    // try to retrieve the build info for first root, which is the most common use
    // case.
    let maybe_tsbuildinfo = if self.options.reload {
      None
    } else {
      self.type_check_cache.get_tsbuildinfo(first_root)
    };
    // to make tsc build info work, we need to consistently hash modules, so that
    // tsc can better determine if an emit is still valid or not, so we provide
    // that data here.
    let compiler_options = check_group.compiler_options.clone();

    let compiler_options_hash_data = FastInsecureHasher::new_deno_versioned()
      .write_hashable(&compiler_options)
      .finish();
    let code_cache = self.code_cache.as_ref().map(|c| {
      let c: Arc<dyn deno_runtime::code_cache::CodeCache> = c.clone();
      c
    });
    let response = tsc::exec(
      tsc::Request {
        config: compiler_options,
        debug: self.log_level == Some(log::Level::Debug),
        graph: self.graph.clone(),
        jsx_import_source_config_resolver: self
          .jsx_import_source_config_resolver
          .clone(),
        hash_data: compiler_options_hash_data,
        maybe_npm: Some(tsc::RequestNpmState {
          cjs_tracker: self.cjs_tracker.clone(),
          node_resolver: self.node_resolver.clone(),
          npm_resolver: self.npm_resolver.clone(),
        }),
        maybe_tsbuildinfo,
        root_names,
        check_mode: self.options.type_check_mode,
        initial_cwd: self.initial_cwd.clone(),
        capture_emitted_files: false,
      },
      code_cache,
    )?;

    let ambient_modules = response.ambient_modules;
    log::debug!("Ambient Modules: {:?}", ambient_modules);

    let ambient_modules_regex = if ambient_modules.is_empty() {
      None
    } else {
      regex::Regex::new(&ambient_modules_to_regex_string(&ambient_modules))
        .inspect_err(|e| {
          log::warn!("Failed to create regex for ambient modules: {}", e);
        })
        .ok()
    };

    let mut response_diagnostics = response.diagnostics.filter(|d| {
      self.should_include_diagnostic(self.options.type_check_mode, d)
        && !self.is_untagged_jsdoc_dynamic_import_diagnostic(d)
    });
    response_diagnostics.apply_fast_check_source_maps(&self.graph);
    response_diagnostics.retain(|d| {
      !is_used_ts_expect_error_diagnostic(d, &used_ts_expect_error_directives)
    });
    let mut diagnostics = missing_diagnostics.filter(|d| {
      if let Some(ambient_modules_regex) = &ambient_modules_regex
        && let Some(missing_specifier) = &d.missing_specifier
      {
        return !ambient_modules_regex.is_match(missing_specifier);
      }
      true
    });
    diagnostics.extend(response_diagnostics);

    if let Some(tsbuildinfo) = response.maybe_tsbuildinfo {
      self
        .type_check_cache
        .set_tsbuildinfo(first_root, &tsbuildinfo);
    }

    if !diagnostics.has_diagnostic()
      && let Some(check_hash) = maybe_check_hash
    {
      self.type_check_cache.add_check_hash(check_hash);
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

  fn is_untagged_jsdoc_dynamic_import_diagnostic(
    &self,
    d: &tsc::Diagnostic,
  ) -> bool {
    if d.code != 2307 {
      return false;
    }
    let Some(file_name) = &d.file_name else {
      return false;
    };
    let Ok(specifier) = ModuleSpecifier::parse(file_name) else {
      return false;
    };
    let Ok(Some(Module::Js(module))) = self.graph.try_get(&specifier) else {
      return false;
    };
    if !matches!(
      module.media_type,
      MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs | MediaType::Jsx
    ) {
      return false;
    }
    let Some(start) = &d.start else {
      return false;
    };
    is_untagged_jsdoc_dynamic_import_position(
      &module.source.text,
      deno_graph::Position::new(start.line as usize, start.character as usize),
    )
  }

  fn add_jsx_runtime_types(
    &self,
    graph_walker: &mut GraphWalker,
    check_group: &CheckGroup,
  ) {
    // Check each root to see if it has a jsxImportSource config.
    // If so, resolve the jsx-runtime types and add to roots.
    let mut seen_jsx_sources = HashSet::new();
    for root in &check_group.roots {
      let Some(jsx_config) =
        self.jsx_import_source_config_resolver.for_specifier(root)
      else {
        continue;
      };
      let Some(specifier) = jsx_config.specifier() else {
        continue;
      };
      if !seen_jsx_sources.insert(specifier.to_string()) {
        continue;
      }
      // Construct the jsx-runtime specifier (e.g., "npm:react/jsx-runtime")
      let jsx_runtime_specifier = format!("{specifier}/jsx-runtime");
      let Ok(npm_ref) = deno_semver::npm::NpmPackageReqReference::from_str(
        &jsx_runtime_specifier,
      ) else {
        continue;
      };
      // Try to resolve the package folder and then the subpath
      let Ok(pkg_folder) = self
        .npm_resolver
        .resolve_pkg_folder_from_deno_module_req(npm_ref.req(), root)
      else {
        continue;
      };
      let Ok(resolved) =
        self.node_resolver.resolve_package_subpath_from_deno_module(
          &pkg_folder,
          npm_ref.sub_path(),
          Some(root),
          node_resolver::ResolutionMode::Import,
          node_resolver::NodeResolutionKind::Types,
        )
      else {
        continue;
      };
      if let Ok(url) = resolved.into_url() {
        let mt = MediaType::from_specifier(&url);
        graph_walker.roots.push((url, mt));
      }
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

pub(crate) struct TscRoots {
  pub(crate) roots: Vec<(ModuleSpecifier, MediaType)>,
  pub(crate) missing_diagnostics: tsc::Diagnostics,
  used_ts_expect_error_directives: HashSet<TsDirective>,
  pub(crate) maybe_check_hash: Option<CacheDBHash>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TsDirective {
  // The module specifier as a graph URL string. Recorded from
  // `deno_graph::Range::specifier` and later matched against
  // `tsc::Diagnostic::file_name`, which is the same canonical form.
  specifier: String,
  // 0-indexed source line of the directive comment. This must use the same
  // line numbering as both `deno_graph::Range::start::line` (where the
  // directive is recorded) and `tsc::Position::line` (where the TS2578 lookup
  // happens) so the two sides match. Both are 0-indexed.
  line: u64,
}

enum TsSuppressionComment {
  Ignore,
  ExpectError(TsDirective),
}

struct GraphWalker<'a> {
  graph: &'a ModuleGraph,
  sys: &'a CliSys,
  node_resolver: &'a CliNodeResolver,
  npm_resolver: &'a CliNpmResolver,
  compiler_options_resolver: &'a CompilerOptionsResolver,
  /// Names of packages importable by bare specifier (workspace members and
  /// packages linked via the "links" field), used to enhance import errors.
  bare_importable_pkg_names: &'a [String],
  maybe_hasher: Option<FastInsecureHasher>,
  seen: HashSet<&'a Url>,
  pending: VecDeque<PendingGraphWalkSpecifier<'a>>,
  has_seen_node_builtin: bool,
  roots: Vec<(ModuleSpecifier, MediaType)>,
  missing_diagnostics: tsc::Diagnostics,
  used_ts_expect_error_directives: HashSet<TsDirective>,
}

struct PendingGraphWalkSpecifier<'a> {
  specifier: &'a Url,
  is_dynamic: bool,
  is_root: bool,
}

impl<'a> GraphWalker<'a> {
  #[allow(clippy::too_many_arguments, reason = "construction")]
  pub fn new(
    graph: &'a ModuleGraph,
    sys: &'a CliSys,
    node_resolver: &'a CliNodeResolver,
    npm_resolver: &'a CliNpmResolver,
    compiler_options_resolver: &'a CompilerOptionsResolver,
    bare_importable_pkg_names: &'a [String],
    npm_cache_state_hash: Option<u64>,
    compiler_options: &CompilerOptions,
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
      hasher.write_hashable(compiler_options);
      hasher
    });
    Self {
      graph,
      sys,
      node_resolver,
      npm_resolver,
      compiler_options_resolver,
      bare_importable_pkg_names,
      maybe_hasher,
      seen: HashSet::with_capacity(
        graph.imports.len() + graph.specifiers_count(),
      ),
      pending: VecDeque::new(),
      has_seen_node_builtin: false,
      roots: Vec::with_capacity(graph.imports.len() + graph.specifiers_count()),
      missing_diagnostics: Default::default(),
      used_ts_expect_error_directives: Default::default(),
    }
  }

  pub fn add_config_import(&mut self, specifier: &'a Url, referrer: &Url) {
    let specifier = self.graph.resolve(specifier);
    if self.seen.insert(specifier) {
      match NpmPackageReqReference::from_specifier(specifier) {
        Ok(req_ref) => match self.resolve_npm_req_ref(&req_ref, referrer) {
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
        },
        _ => {
          self.pending.push_back(PendingGraphWalkSpecifier {
            specifier,
            is_dynamic: false,
            is_root: false,
          });
          self.resolve_pending();
        }
      }
    }
  }

  pub fn add_root(&mut self, root: &'a Url) {
    let specifier = self.graph.resolve(root);
    if self.seen.insert(specifier) {
      self.pending.push_back(PendingGraphWalkSpecifier {
        specifier,
        is_dynamic: false,
        is_root: true,
      });
    }

    self.resolve_pending()
  }

  /// Transform the graph into root specifiers that we can feed `tsc`. We have to
  /// provide the media type for root modules because `tsc` does not "resolve" the
  /// media type like other modules, as well as a root specifier needs any
  /// redirects resolved. We need to include all the emittable files in
  /// the roots, so they get type checked and optionally emitted,
  /// otherwise they would be ignored if only imported into JavaScript.
  pub fn into_tsc_roots(mut self) -> TscRoots {
    if self.has_seen_node_builtin && !self.roots.is_empty() {
      // inject a specifier that will force node types to be resolved
      self.roots.push((
        ModuleSpecifier::parse("asset:///reference_types_node.d.ts").unwrap(),
        MediaType::Dts,
      ));
    }
    TscRoots {
      roots: self.roots,
      missing_diagnostics: self.missing_diagnostics,
      used_ts_expect_error_directives: self.used_ts_expect_error_directives,
      maybe_check_hash: self.maybe_hasher.map(|h| CacheDBHash::new(h.finish())),
    }
  }

  fn source_text_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&str> {
    self
      .graph
      .try_get_prefer_types(specifier)
      .ok()
      .flatten()
      .and_then(|m| m.js())
      .map(|m| m.source.text.as_ref())
  }

  fn maybe_ts_suppression_comment(
    &self,
    range: &deno_graph::Range,
  ) -> Option<TsSuppressionComment> {
    maybe_ts_suppression_comment(
      range.specifier.as_str(),
      self.source_text_for_specifier(&range.specifier)?,
      range.range.start.line,
    )
  }

  fn push_missing_diagnostic(
    &mut self,
    diagnostic: tsc::Diagnostic,
    maybe_range: Option<&deno_graph::Range>,
  ) {
    if let Some(range) = maybe_range
      && let Some(comment) = self.maybe_ts_suppression_comment(range)
    {
      if let TsSuppressionComment::ExpectError(directive) = comment {
        self.used_ts_expect_error_directives.insert(directive);
      }
      return;
    }
    self.missing_diagnostics.push(diagnostic);
  }

  fn resolve_pending(&mut self) {
    while let Some(PendingGraphWalkSpecifier {
      specifier,
      is_dynamic,
      is_root,
    }) = self.pending.pop_front()
    {
      let module = match self.graph.try_get(specifier) {
        Ok(Some(module)) => module,
        Ok(None) => continue,
        Err(err) => {
          if !is_dynamic
            && let Some(err) = module_error_for_tsc_diagnostic(self.sys, err)
          {
            self.push_missing_diagnostic(
              tsc::Diagnostic::from_missing_error(
                err.specifier.as_str(),
                err.maybe_range,
                maybe_additional_sloppy_imports_message(
                  self.sys,
                  err.specifier,
                ),
              ),
              err.maybe_range,
            );
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
      let is_js_module = matches!(
        module.media_type(),
        MediaType::JavaScript
          | MediaType::Mjs
          | MediaType::Cjs
          | MediaType::Jsx
      );

      let mut maybe_module_dependencies = None;
      let mut maybe_types_dependency = None;
      let mut maybe_js_source_text = None;
      match module {
        Module::Js(module) => {
          maybe_module_dependencies =
            Some(module.dependencies_prefer_fast_check());
          maybe_js_source_text = Some(module.source.text.as_ref());
          maybe_types_dependency = module
            .maybe_types_dependency
            .as_ref()
            .and_then(|d| d.dependency.ok());
        }
        Module::Wasm(module) => {
          maybe_module_dependencies = Some(&module.dependencies);
        }
        Module::Json(_) | Module::Npm(_) => {}
        Module::External(module) => {
          // NPM files for `"nodeModulesDir": "manual"`.
          let media_type = MediaType::from_specifier(&module.specifier);
          if media_type.is_declaration() {
            self.roots.push((module.specifier.clone(), media_type));
          }
        }
        Module::Node(_) => {
          if !self.has_seen_node_builtin {
            self.has_seen_node_builtin = true;
          }
        }
      }

      if module.media_type().is_declaration() {
        // When a `.d.ts` is itself a check root (an explicit entrypoint), its
        // own unresolved imports should surface as `TS2307`. deno_graph records
        // a bare specifier in a `.d.ts` as `Resolution::None`, which both the
        // missing-import loop below and tsc (under `skipLibCheck`) ignore, so
        // handle it explicitly here regardless of `skipLibCheck`. Dependency
        // `.d.ts` files reached transitively are not roots and keep being
        // skipped under `skipLibCheck`.
        if is_root && let Module::Js(module) = module {
          self.add_unresolved_dts_entrypoint_imports(module);
        }
        let compiler_options_data = self
          .compiler_options_resolver
          .for_specifier(module.specifier());
        if compiler_options_data.skip_lib_check() {
          continue;
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
              deno_graph::Resolution::Err(_) | deno_graph::Resolution::None => {
              }
            }
          }
          if dep.is_dynamic {
            continue;
          }
          if is_js_module && dep.maybe_code.is_none() {
            continue;
          }
          // only surface the code error if there's no type
          let dep_to_check_error = if dep.maybe_type.is_none() {
            &dep.maybe_code
          } else {
            &dep.maybe_type
          };
          if let deno_graph::Resolution::Err(resolution_error) =
            dep_to_check_error
            && !(is_js_module
              && maybe_js_source_text.is_some_and(|text| {
                is_untagged_jsdoc_dynamic_import_range(
                  text,
                  resolution_error.range(),
                )
              }))
            && let Some(diagnostic) =
              tsc::Diagnostic::maybe_from_resolution_error(
                resolution_error,
                self.bare_importable_pkg_names,
              )
          {
            self.push_missing_diagnostic(
              diagnostic,
              Some(resolution_error.range()),
            );
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
              .compiler_options_resolver
              .for_specifier(&module.specifier)
              .check_js()
              || has_ts_check(module.media_type, &module.source.text)
            {
              Some((module.specifier.clone(), module.media_type))
            } else {
              None
            }
          }
          MediaType::Json
          | MediaType::Jsonc
          | MediaType::Json5
          | MediaType::Wasm
          | MediaType::Css
          | MediaType::Html
          | MediaType::Markdown
          | MediaType::SourceMap
          | MediaType::Sql
          | MediaType::Unknown => None,
        };
        if result.is_some()
          && let Some(hasher) = &mut self.maybe_hasher
        {
          hasher.write_str(module.specifier.as_str());
          hasher.write_str(
            // the fast check module will only be set when publishing
            module
              .fast_check_module()
              .map(|s| s.source.as_ref())
              .unwrap_or(&module.source.text),
          );
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
          hasher.write_str(&module.source.text);
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
        self.pending.push_back(PendingGraphWalkSpecifier {
          specifier,
          is_dynamic: true,
          is_root: false,
        });
      }
    } else if self.seen.insert(specifier) {
      self.pending.push_back(PendingGraphWalkSpecifier {
        specifier,
        is_dynamic: false,
        is_root: false,
      });
    }
  }

  /// Surface unresolved imports of a `.d.ts` check root as `TS2307`.
  ///
  /// An explicit `.d.ts` entrypoint should report its own unresolved imports,
  /// but deno_graph records a bare specifier in a `.d.ts` as `Resolution::None`
  /// (a `.ts` file records `Resolution::Err`). The missing-import loop only
  /// turns `Resolution::Err` into diagnostics, so a `.d.ts` entrypoint's bare
  /// imports are otherwise swallowed whether or not `skipLibCheck` is set.
  ///
  /// Only `Resolution::None` deps are handled here. Resolved (`Ok`) and errored
  /// (`Err`) deps are already surfaced by the missing-import loop when
  /// `skipLibCheck` is off, so handling them here too would double report. The
  /// import range deno_graph already parsed is reused, so there's no need to
  /// re-parse the source.
  fn add_unresolved_dts_entrypoint_imports(
    &mut self,
    module: &'a deno_graph::JsModule,
  ) {
    for dep in module.dependencies_prefer_fast_check().values() {
      // Only handle imports deno_graph left fully unresolved; a bare specifier
      // in a `.d.ts` lands here as `None`/`None`.
      if !matches!(dep.maybe_code, deno_graph::Resolution::None)
        || !matches!(dep.maybe_type, deno_graph::Resolution::None)
      {
        continue;
      }
      for import in &dep.imports {
        // Only surface real `import`/`export`/`import =` statements. Triple
        // slash `/// <reference />` directives, JSDoc and `@jsxImportSource`
        // imports are intentionally left to tsc's `skipLibCheck` handling.
        if !matches!(
          import.kind,
          deno_graph::ImportKind::Es
            | deno_graph::ImportKind::TsType
            | deno_graph::ImportKind::Require
        ) {
          continue;
        }
        let range = import.specifier_range.clone();
        match deno_path_util::resolve_import(
          &import.specifier,
          &module.specifier,
        ) {
          Ok(specifier) => {
            let specifier = self.graph.resolve(&specifier);
            if self.graph.try_get(specifier).ok().flatten().is_none() {
              self.missing_diagnostics.push(
                tsc::Diagnostic::from_missing_error(
                  specifier.as_str(),
                  Some(&range),
                  maybe_additional_sloppy_imports_message(self.sys, specifier),
                ),
              );
            }
          }
          Err(error) => {
            let resolution_error =
              ResolutionError::InvalidSpecifier { error, range };
            if let Some(diagnostic) =
              tsc::Diagnostic::maybe_from_resolution_error(
                &resolution_error,
                self.bare_importable_pkg_names,
              )
            {
              self.missing_diagnostics.push(diagnostic);
            }
          }
        }
      }
    }
  }

  fn resolve_npm_req_ref(
    &self,
    req_ref: &NpmPackageReqReference,
    referrer: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let pkg_dir = self
      .npm_resolver
      .as_managed()
      .unwrap()
      .resolve_pkg_folder_from_deno_module_req(req_ref.req())
      .ok()?;
    let resolved = self
      .node_resolver
      .resolve_package_subpath_from_deno_module(
        &pkg_dir,
        req_ref.sub_path(),
        Some(referrer),
        node_resolver::ResolutionMode::Import,
        node_resolver::NodeResolutionKind::Types,
      )
      .ok()?;
    resolved.into_url().ok()
  }
}

fn is_used_ts_expect_error_diagnostic(
  diagnostic: &tsc::Diagnostic,
  used_ts_expect_error_directives: &HashSet<TsDirective>,
) -> bool {
  const TS_UNUSED_EXPECT_ERROR: u64 = 2578;
  if diagnostic.code != TS_UNUSED_EXPECT_ERROR {
    return false;
  }
  let Some(file_name) = &diagnostic.file_name else {
    return false;
  };
  // Prefer `original_source_start`: `apply_fast_check_source_maps` may have
  // rewritten `start` to point into generated fast-check output, whereas the
  // directive was recorded against the original source position. This mirrors
  // how the diagnostic's own display picks its position.
  let Some(position) = diagnostic
    .original_source_start
    .as_ref()
    .or(diagnostic.start.as_ref())
  else {
    return false;
  };
  used_ts_expect_error_directives.contains(&TsDirective {
    specifier: file_name.to_string(),
    line: position.line,
  })
}

/// Looks for a `@ts-ignore` / `@ts-expect-error` directive suppressing a
/// diagnostic recorded at `diagnostic_line` (0-indexed).
///
/// The graph records a missing-module diagnostic at the *specifier*, which for
/// a multi-line `import`/`export ... from "..."` sits below the statement
/// start. tsc instead reports its own diagnostics at the statement start and
/// anchors a preceding directive to that line, so we first walk up to the line
/// that begins the import/export statement.
///
/// From there this mirrors TypeScript's own `markPrecedingCommentDirectiveLine`:
/// starting on the line above the statement, scan upwards skipping blank lines
/// and any `//` comment lines, and stop at the first line of actual code. The
/// directive only needs to be the nearest comment above the statement, not
/// strictly on the immediately preceding line. Keeping this in sync with tsc
/// matters so a graph-derived missing-module diagnostic is suppressed in
/// exactly the cases tsc would suppress its own diagnostics.
fn maybe_ts_suppression_comment(
  specifier: &str,
  source_text: &str,
  diagnostic_line: usize,
) -> Option<TsSuppressionComment> {
  // We only ever scan upward from the diagnostic, so there's no need to
  // materialize the rest of the file (imports sit near the top).
  let lines = source_text
    .lines()
    .take(diagnostic_line + 1)
    .collect::<Vec<_>>();

  // Walk up from the specifier to the line that begins its import/export
  // statement. A bare comment line (e.g. a `/// <reference />`) or anything
  // that isn't a resolvable multi-line import body falls back to the
  // diagnostic line, preserving the single-line behavior.
  let mut anchor = diagnostic_line;
  loop {
    let trimmed = lines.get(anchor)?.trim_start();
    if trimmed.starts_with("import") || trimmed.starts_with("export") {
      break;
    }
    if anchor == 0 || trimmed.is_empty() || trimmed.starts_with("//") {
      anchor = diagnostic_line;
      break;
    }
    anchor -= 1;
  }

  let mut line_index = anchor.checked_sub(1)?;
  loop {
    let line = lines.get(line_index)?.trim();
    if let Some(directive) = line.strip_prefix("//").and_then(|line| {
      line
        .strip_prefix('/')
        .unwrap_or(line)
        .trim_start()
        .strip_prefix('@')
    }) {
      if directive.starts_with("ts-ignore") {
        return Some(TsSuppressionComment::Ignore);
      }
      if directive.starts_with("ts-expect-error") {
        return Some(TsSuppressionComment::ExpectError(TsDirective {
          specifier: specifier.to_string(),
          line: line_index as u64,
        }));
      }
    }

    if !line.is_empty() && !line.starts_with("//") {
      return None;
    }

    line_index = line_index.checked_sub(1)?;
  }
}

static JSDOC_DYNAMIC_IMPORT_RE: Lazy<Regex> =
  lazy_regex::lazy_regex!(r#"(?s)(?:^|[^\w$])import\s*\(\s*["'][^"']+["']"#);
static JSDOC_TYPED_TAG_RE: Lazy<Regex> = lazy_regex::lazy_regex!(
  r#"@(?:augments|extends|implements|import|param|returns?|satisfies|template|typedef|type)\b"#
);

fn is_untagged_jsdoc_dynamic_import_range(
  text: &str,
  range: &deno_graph::Range,
) -> bool {
  is_untagged_jsdoc_dynamic_import_position(text, range.range.start)
}

fn is_untagged_jsdoc_dynamic_import_position(
  text: &str,
  position: deno_graph::Position,
) -> bool {
  let Some(start) = position_to_byte_index(text, position) else {
    return false;
  };
  let Some(comment_start) = text[..start].rfind("/**") else {
    return false;
  };
  if text[..start]
    .rfind("*/")
    .is_some_and(|comment_end| comment_end > comment_start)
  {
    return false;
  }

  let Some(open_brace) = text[..start].rfind('{') else {
    return false;
  };
  if open_brace <= comment_start
    || text[..start]
      .rfind('}')
      .is_some_and(|close_brace| close_brace > open_brace)
  {
    return false;
  }
  if JSDOC_TYPED_TAG_RE.is_match(&text[comment_start..open_brace]) {
    return false;
  }

  let Some(close_brace) = text[start..].find('}').map(|i| start + i) else {
    return false;
  };
  if text[start..]
    .find("*/")
    .is_some_and(|comment_end| start + comment_end < close_brace)
  {
    return false;
  }

  JSDOC_DYNAMIC_IMPORT_RE.is_match(&text[open_brace + 1..close_brace])
}

fn position_to_byte_index(
  text: &str,
  position: deno_graph::Position,
) -> Option<usize> {
  let mut line = 0;
  let mut character = 0;
  for (index, c) in text.char_indices() {
    if line == position.line && character == position.character {
      return Some(index);
    }
    if c == '\n' {
      line += 1;
      character = 0;
    } else {
      character += 1;
    }
  }
  (line == position.line && character == position.character)
    .then_some(text.len())
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
    | MediaType::Jsonc
    | MediaType::Json5
    | MediaType::Markdown
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

  use super::ambient_modules_to_regex_string;
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

  #[test]
  fn ambient_modules_to_regex_string_test() {
    let result = ambient_modules_to_regex_string(&[
      "foo".to_string(),
      "*.css".to_string(),
      "$virtual/module".to_string(),
    ]);
    assert_eq!(result, r"^(foo|.*\.css|\$virtual/module)$");
  }
}
