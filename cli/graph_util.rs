// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::Lockfile;
use crate::args::TsConfigType;
use crate::args::TsTypeLib;
use crate::args::TypeCheckMode;
use crate::cache;
use crate::cache::TypeCheckCache;
use crate::colors;
use crate::errors::get_error_class_name;
use crate::npm::NpmPackageResolver;
use crate::proc_state::ProcState;
use crate::resolver::CliGraphResolver;
use crate::tools::check;

use deno_core::anyhow::bail;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::RwLock;
use deno_core::ModuleSpecifier;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ResolutionError;
use deno_graph::SpecifierError;
use deno_runtime::permissions::PermissionsContainer;
use import_map::ImportMapError;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

#[derive(Clone, Copy)]
pub struct GraphValidOptions {
  pub check_js: bool,
  pub follow_type_only: bool,
  pub is_vendoring: bool,
}

/// Check if `roots` and their deps are available. Returns `Ok(())` if
/// so. Returns `Err(_)` if there is a known module graph or resolution
/// error statically reachable from `roots` and not a dynamic import.
pub fn graph_valid_with_cli_options(
  graph: &ModuleGraph,
  roots: &[ModuleSpecifier],
  options: &CliOptions,
) -> Result<(), AnyError> {
  graph_valid(
    graph,
    roots,
    GraphValidOptions {
      is_vendoring: false,
      follow_type_only: options.type_check_mode() != TypeCheckMode::None,
      check_js: options.check_js(),
    },
  )
}

/// Check if `roots` and their deps are available. Returns `Ok(())` if
/// so. Returns `Err(_)` if there is a known module graph or resolution
/// error statically reachable from `roots`.
///
/// It is preferable to use this over using deno_graph's API directly
/// because it will have enhanced error message information specifically
/// for the CLI.
pub fn graph_valid(
  graph: &ModuleGraph,
  roots: &[ModuleSpecifier],
  options: GraphValidOptions,
) -> Result<(), AnyError> {
  let mut errors = graph
    .walk(
      roots,
      deno_graph::WalkOptions {
        check_js: options.check_js,
        follow_type_only: options.follow_type_only,
        follow_dynamic: options.is_vendoring,
      },
    )
    .errors()
    .flat_map(|error| {
      let is_root = match &error {
        ModuleGraphError::ResolutionError(_) => false,
        _ => roots.contains(error.specifier()),
      };
      let mut message = if let ModuleGraphError::ResolutionError(err) = &error {
        enhanced_resolution_error_message(err)
      } else {
        format!("{error}")
      };

      if let Some(range) = error.maybe_range() {
        if !is_root && !range.specifier.as_str().contains("/$deno$eval") {
          message.push_str(&format!("\n    at {range}"));
        }
      }

      if options.is_vendoring {
        // warn about failing dynamic imports when vendoring, but don't fail completely
        if matches!(error, ModuleGraphError::MissingDynamic(_, _)) {
          log::warn!("Ignoring: {:#}", message);
          return None;
        }

        // ignore invalid downgrades and invalid local imports when vendoring
        if let ModuleGraphError::ResolutionError(err) = &error {
          if matches!(
            err,
            ResolutionError::InvalidDowngrade { .. }
              | ResolutionError::InvalidLocalImport { .. }
          ) {
            return None;
          }
        }
      }

      Some(custom_error(get_error_class_name(&error.into()), message))
    });
  if let Some(error) = errors.next() {
    Err(error)
  } else {
    Ok(())
  }
}

/// Checks the lockfile against the graph and and exits on errors.
pub fn graph_lock_or_exit(graph: &ModuleGraph, lockfile: &mut Lockfile) {
  for module in graph.modules() {
    let source = match module {
      Module::Esm(module) => &module.source,
      Module::Json(module) => &module.source,
      Module::Node(_) | Module::Npm(_) | Module::External(_) => continue,
    };
    if !lockfile.check_or_insert_remote(module.specifier().as_str(), source) {
      let err = format!(
        concat!(
          "The source code is invalid, as it does not match the expected hash in the lock file.\n",
          "  Specifier: {}\n",
          "  Lock file: {}",
        ),
        module.specifier(),
        lockfile.filename.display(),
      );
      log::error!("{} {}", colors::red("error:"), err);
      std::process::exit(10);
    }
  }
}

pub async fn create_graph_and_maybe_check(
  root: ModuleSpecifier,
  ps: &ProcState,
) -> Result<Arc<deno_graph::ModuleGraph>, AnyError> {
  let mut cache = cache::FetchCacher::new(
    ps.emit_cache.clone(),
    ps.file_fetcher.clone(),
    PermissionsContainer::allow_all(),
    PermissionsContainer::allow_all(),
    ps.options.node_modules_dir_specifier(),
  );
  let maybe_imports = ps.options.to_maybe_imports()?;
  let cli_resolver = CliGraphResolver::new(
    ps.options.to_maybe_jsx_import_source_config(),
    ps.maybe_import_map.clone(),
    ps.options.no_npm(),
    ps.npm_resolver.api().clone(),
    ps.npm_resolver.resolution().clone(),
    ps.package_json_deps_installer.clone(),
  );
  let graph_resolver = cli_resolver.as_graph_resolver();
  let graph_npm_resolver = cli_resolver.as_graph_npm_resolver();
  let analyzer = ps.parsed_source_cache.as_analyzer();
  let mut graph = ModuleGraph::default();
  build_graph_with_npm_resolution(
    &mut graph,
    &ps.npm_resolver,
    vec![root],
    &mut cache,
    deno_graph::BuildOptions {
      is_dynamic: false,
      imports: maybe_imports,
      resolver: Some(graph_resolver),
      npm_resolver: Some(graph_npm_resolver),
      module_analyzer: Some(&*analyzer),
      reporter: None,
    },
  )
  .await?;

  graph_valid_with_cli_options(&graph, &graph.roots, &ps.options)?;
  let graph = Arc::new(graph);
  if let Some(lockfile) = &ps.lockfile {
    graph_lock_or_exit(&graph, &mut lockfile.lock());
  }

  if ps.options.type_check_mode() != TypeCheckMode::None {
    // node built-in specifiers use the @types/node package to determine
    // types, so inject that now after the lockfile has been written
    if graph.has_node_specifier {
      ps.npm_resolver
        .inject_synthetic_types_node_package()
        .await?;
    }

    let ts_config_result =
      ps.options.resolve_ts_config_for_emit(TsConfigType::Check {
        lib: ps.options.ts_type_lib_window(),
      })?;
    if let Some(ignored_options) = ts_config_result.maybe_ignored_options {
      log::warn!("{}", ignored_options);
    }
    let maybe_config_specifier = ps.options.maybe_config_file_specifier();
    let cache = TypeCheckCache::new(&ps.dir.type_checking_cache_db_file_path());
    let check_result = check::check(
      graph.clone(),
      &cache,
      &ps.npm_resolver,
      check::CheckOptions {
        type_check_mode: ps.options.type_check_mode(),
        debug: ps.options.log_level() == Some(log::Level::Debug),
        maybe_config_specifier,
        ts_config: ts_config_result.ts_config,
        log_checks: true,
        reload: ps.options.reload_flag(),
      },
    )?;
    log::debug!("{}", check_result.stats);
    if !check_result.diagnostics.is_empty() {
      return Err(check_result.diagnostics.into());
    }
  }

  Ok(graph)
}

pub async fn build_graph_with_npm_resolution<'a>(
  graph: &mut ModuleGraph,
  npm_resolver: &NpmPackageResolver,
  roots: Vec<ModuleSpecifier>,
  loader: &mut dyn deno_graph::source::Loader,
  options: deno_graph::BuildOptions<'a>,
) -> Result<(), AnyError> {
  graph.build(roots, loader, options).await;

  // resolve the dependencies of any pending dependencies
  // that were inserted by building the graph
  npm_resolver.resolve_pending().await?;

  Ok(())
}

pub fn error_for_any_npm_specifier(
  graph: &ModuleGraph,
) -> Result<(), AnyError> {
  for module in graph.modules() {
    match module {
      Module::Npm(module) => {
        bail!("npm specifiers have not yet been implemented for this sub command (https://github.com/denoland/deno/issues/15960). Found: {}", module.specifier)
      }
      Module::Node(module) => {
        bail!("Node specifiers have not yet been implemented for this sub command (https://github.com/denoland/deno/issues/15960). Found: node:{}", module.module_name)
      }
      Module::Esm(_) | Module::Json(_) | Module::External(_) => {}
    }
  }
  Ok(())
}

/// Adds more explanatory information to a resolution error.
pub fn enhanced_resolution_error_message(error: &ResolutionError) -> String {
  let mut message = format!("{error}");

  if let Some(specifier) = get_resolution_error_bare_node_specifier(error) {
    message.push_str(&format!(
        "\nIf you want to use a built-in Node module, add a \"node:\" prefix (ex. \"node:{specifier}\")."
      ));
  }

  message
}

pub fn get_resolution_error_bare_node_specifier(
  error: &ResolutionError,
) -> Option<&str> {
  get_resolution_error_bare_specifier(error).filter(|specifier| {
    crate::node::resolve_builtin_node_module(specifier).is_ok()
  })
}

fn get_resolution_error_bare_specifier(
  error: &ResolutionError,
) -> Option<&str> {
  if let ResolutionError::InvalidSpecifier {
    error: SpecifierError::ImportPrefixMissing(specifier, _),
    ..
  } = error
  {
    Some(specifier.as_str())
  } else if let ResolutionError::ResolverError { error, .. } = error {
    if let Some(ImportMapError::UnmappedBareSpecifier(specifier, _)) =
      error.downcast_ref::<ImportMapError>()
    {
      Some(specifier.as_str())
    } else {
      None
    }
  } else {
    None
  }
}

#[derive(Default, Debug)]
struct GraphData {
  graph: Arc<ModuleGraph>,
  checked_libs: HashMap<TsTypeLib, HashSet<ModuleSpecifier>>,
}

/// Holds the `ModuleGraph` and what parts of it are type checked.
#[derive(Clone)]
pub struct ModuleGraphContainer {
  update_semaphore: Arc<Semaphore>,
  graph_data: Arc<RwLock<GraphData>>,
}

impl Default for ModuleGraphContainer {
  fn default() -> Self {
    Self {
      update_semaphore: Arc::new(Semaphore::new(1)),
      graph_data: Default::default(),
    }
  }
}

impl ModuleGraphContainer {
  /// Acquires a permit to modify the module graph without other code
  /// having the chance to modify it. In the meantime, other code may
  /// still read from the existing module graph.
  pub async fn acquire_update_permit(&self) -> ModuleGraphUpdatePermit {
    let permit = self.update_semaphore.acquire().await.unwrap();
    ModuleGraphUpdatePermit {
      permit,
      graph_data: self.graph_data.clone(),
      graph: (*self.graph_data.read().graph).clone(),
    }
  }

  pub fn graph(&self) -> Arc<ModuleGraph> {
    self.graph_data.read().graph.clone()
  }

  /// Mark `roots` and all of their dependencies as type checked under `lib`.
  /// Assumes that all of those modules are known.
  pub fn set_type_checked(&self, roots: &[ModuleSpecifier], lib: TsTypeLib) {
    // It's ok to analyze and update this while the module graph itself is
    // being updated in a permit because the module graph update is always
    // additive and this will be a subset of the original graph
    let graph = self.graph();
    let entries = graph.walk(
      roots,
      deno_graph::WalkOptions {
        check_js: true,
        follow_dynamic: true,
        follow_type_only: true,
      },
    );

    // now update
    let mut data = self.graph_data.write();
    let checked_lib_set = data.checked_libs.entry(lib).or_default();
    for (specifier, _) in entries {
      checked_lib_set.insert(specifier.clone());
    }
  }

  /// Check if `roots` are all marked as type checked under `lib`.
  pub fn is_type_checked(
    &self,
    roots: &[ModuleSpecifier],
    lib: TsTypeLib,
  ) -> bool {
    let data = self.graph_data.read();
    match data.checked_libs.get(&lib) {
      Some(checked_lib_set) => roots.iter().all(|r| {
        let found = data.graph.resolve(r);
        checked_lib_set.contains(&found)
      }),
      None => false,
    }
  }
}

/// A permit for updating the module graph. When complete and
/// everything looks fine, calling `.commit()` will store the
/// new graph in the ModuleGraphContainer.
pub struct ModuleGraphUpdatePermit<'a> {
  permit: SemaphorePermit<'a>,
  graph_data: Arc<RwLock<GraphData>>,
  graph: ModuleGraph,
}

impl<'a> ModuleGraphUpdatePermit<'a> {
  /// Gets the module graph for mutation.
  pub fn graph_mut(&mut self) -> &mut ModuleGraph {
    &mut self.graph
  }

  /// Saves the mutated module graph in the container
  /// and returns an Arc to the new module graph.
  pub fn commit(self) -> Arc<ModuleGraph> {
    let graph = Arc::new(self.graph);
    self.graph_data.write().graph = graph.clone();
    drop(self.permit); // explicit drop for clarity
    graph
  }
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use deno_ast::ModuleSpecifier;
  use deno_graph::Position;
  use deno_graph::Range;
  use deno_graph::ResolutionError;
  use deno_graph::SpecifierError;

  use crate::graph_util::get_resolution_error_bare_node_specifier;

  #[test]
  fn import_map_node_resolution_error() {
    let cases = vec![("fs", Some("fs")), ("other", None)];
    for (input, output) in cases {
      let import_map = import_map::ImportMap::new(
        ModuleSpecifier::parse("file:///deno.json").unwrap(),
      );
      let specifier = ModuleSpecifier::parse("file:///file.ts").unwrap();
      let err = import_map.resolve(input, &specifier).err().unwrap();
      let err = ResolutionError::ResolverError {
        error: Arc::new(err.into()),
        specifier: input.to_string(),
        range: Range {
          specifier,
          start: Position::zeroed(),
          end: Position::zeroed(),
        },
      };
      assert_eq!(get_resolution_error_bare_node_specifier(&err), output);
    }
  }

  #[test]
  fn bare_specifier_node_resolution_error() {
    let cases = vec![("process", Some("process")), ("other", None)];
    for (input, output) in cases {
      let specifier = ModuleSpecifier::parse("file:///file.ts").unwrap();
      let err = ResolutionError::InvalidSpecifier {
        range: Range {
          specifier,
          start: Position::zeroed(),
          end: Position::zeroed(),
        },
        error: SpecifierError::ImportPrefixMissing(input.to_string(), None),
      };
      assert_eq!(get_resolution_error_bare_node_specifier(&err), output,);
    }
  }
}
