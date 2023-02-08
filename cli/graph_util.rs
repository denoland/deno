// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::Lockfile;
use crate::args::TsConfigType;
use crate::args::TsTypeLib;
use crate::args::TypeCheckMode;
use crate::cache;
use crate::cache::TypeCheckCache;
use crate::colors;
use crate::errors::get_error_class_name;
use crate::npm::resolve_graph_npm_info;
use crate::npm::NpmPackageReference;
use crate::npm::NpmPackageReq;
use crate::proc_state::ProcState;
use crate::resolver::CliResolver;
use crate::tools::check;

use deno_core::anyhow::bail;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ResolutionError;
use deno_graph::SpecifierError;
use deno_graph::WalkOptions;
use deno_runtime::permissions::PermissionsContainer;
use import_map::ImportMapError;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct GraphData {
  graph: Arc<ModuleGraph>,
  npm_packages: Vec<NpmPackageReq>,
  has_node_builtin_specifier: bool,
  checked_libs: HashSet<(ModuleSpecifier, TsTypeLib)>,
}

impl GraphData {
  /// Store data from `graph` into `self`.
  pub fn set_graph(&mut self, graph: Arc<ModuleGraph>) {
    let mut has_npm_specifier_in_graph = false;

    for (specifier, _) in graph.specifiers() {
      match specifier.scheme() {
        "node" => {
          self.has_node_builtin_specifier = true;
        }
        "npm" => {
          if !has_npm_specifier_in_graph
            && NpmPackageReference::from_specifier(specifier).is_ok()
          {
            has_npm_specifier_in_graph = true;
          }
        }
        _ => {}
      }

      if has_npm_specifier_in_graph && self.has_node_builtin_specifier {
        break; // exit early
      }
    }

    if has_npm_specifier_in_graph {
      self.npm_packages = resolve_graph_npm_info(&graph).package_reqs;
    }
    self.graph = graph;
  }

  pub fn get_graph(&self) -> &Arc<ModuleGraph> {
    &self.graph
  }

  // todo(dsherret): remove the need for cloning this
  pub fn get_graph_clone(&self) -> ModuleGraph {
    (*self.graph).clone()
  }

  /// Gets if the graph had a "node:" specifier.
  pub fn has_node_builtin_specifier(&self) -> bool {
    self.has_node_builtin_specifier
  }

  /// Gets the npm package requirements from all the encountered graphs
  /// in the order that they should be resolved.
  pub fn npm_package_reqs(&self) -> &Vec<NpmPackageReq> {
    &self.npm_packages
  }

  /// Mark `roots` and all of their dependencies as type checked under `lib`.
  /// Assumes that all of those modules are known.
  pub fn set_type_checked(
    &mut self,
    roots: &[ModuleSpecifier],
    lib: TsTypeLib,
  ) {
    let entries = self.graph.walk(
      roots,
      WalkOptions {
        check_js: true,
        follow_dynamic: true,
        follow_type_only: true,
      },
    );
    for (specifier, _) in entries {
      self.checked_libs.insert((specifier.clone(), lib));
    }
  }

  /// Check if `roots` are all marked as type checked under `lib`.
  pub fn is_type_checked(
    &self,
    roots: &[ModuleSpecifier],
    lib: TsTypeLib,
  ) -> bool {
    roots.iter().all(|r| {
      let found = self.graph.resolve(r);
      let key = (found.clone(), lib);
      self.checked_libs.contains(&key)
    })
  }
}

/// Check if `roots` and their deps are available. Returns `Ok(())` if
/// so. Returns `Err(_)` if there is a known module graph or resolution
/// error statically reachable from `roots`.
pub fn graph_valid(
  graph: &ModuleGraph,
  roots: &[ModuleSpecifier],
  follow_type_only: bool,
  check_js: bool,
) -> Result<(), AnyError> {
  graph
    .walk(
      &roots,
      deno_graph::WalkOptions {
        follow_dynamic: false,
        follow_type_only,
        check_js,
      },
    )
    .validate()
    .map_err(|error| {
      let mut message = if let ModuleGraphError::ResolutionError(err) = &error {
        enhanced_resolution_error_message(err)
      } else {
        format!("{error}")
      };

      if let Some(range) = error.maybe_range() {
        if !range.specifier.as_str().contains("/$deno$eval") {
          message.push_str(&format!("\n    at {}", range));
        }
      }

      custom_error(get_error_class_name(&error.into()), message)
    })
}

/// Checks the lockfile against the graph and and exits on errors.
pub fn graph_lock_or_exit(graph: &ModuleGraph, lockfile: &mut Lockfile) {
  for module in graph.modules() {
    if let Some(source) = &module.maybe_source {
      if !lockfile.check_or_insert_remote(module.specifier.as_str(), source) {
        let err = format!(
          concat!(
            "The source code is invalid, as it does not match the expected hash in the lock file.\n",
            "  Specifier: {}\n",
            "  Lock file: {}",
          ),
          module.specifier,
          lockfile.filename.display(),
        );
        log::error!("{} {}", colors::red("error:"), err);
        std::process::exit(10);
      }
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
  );
  let maybe_imports = ps.options.to_maybe_imports()?;
  let maybe_cli_resolver = CliResolver::maybe_new(
    ps.options.to_maybe_jsx_import_source_config(),
    ps.maybe_import_map.clone(),
  );
  let maybe_graph_resolver =
    maybe_cli_resolver.as_ref().map(|r| r.as_graph_resolver());
  let analyzer = ps.parsed_source_cache.as_analyzer();
  let mut graph = ModuleGraph::default();
  graph
    .build(
      vec![root],
      &mut cache,
      deno_graph::BuildOptions {
        is_dynamic: false,
        imports: maybe_imports,
        resolver: maybe_graph_resolver,
        module_analyzer: Some(&*analyzer),
        reporter: None,
      },
    )
    .await;
  let check_js = ps.options.check_js();
  graph_valid(
    &graph,
    &graph.roots,
    ps.options.type_check_mode() != TypeCheckMode::None,
    check_js,
  )?;
  let graph = Arc::new(graph);
  let (npm_package_reqs, has_node_builtin_specifier) = {
    let mut graph_data = GraphData::default();
    graph_data.set_graph(graph.clone());
    (
      graph_data.npm_package_reqs().clone(),
      graph_data.has_node_builtin_specifier(),
    )
  };
  ps.npm_resolver.add_package_reqs(npm_package_reqs).await?;
  if let Some(lockfile) = &ps.lockfile {
    graph_lock_or_exit(&graph, &mut lockfile.lock());
  }

  if ps.options.type_check_mode() != TypeCheckMode::None {
    // node built-in specifiers use the @types/node package to determine
    // types, so inject that now after the lockfile has been written
    if has_node_builtin_specifier {
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
      &graph.roots,
      &ps.graph_data,
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

pub fn error_for_any_npm_specifier(
  graph: &deno_graph::ModuleGraph,
) -> Result<(), AnyError> {
  let first_npm_specifier = graph
    .specifiers()
    .filter_map(|(_, r)| match r {
      Ok(module) if module.kind == deno_graph::ModuleKind::External => {
        Some(&module.specifier)
      }
      _ => None,
    })
    .next();
  if let Some(npm_specifier) = first_npm_specifier {
    bail!("npm specifiers have not yet been implemented for this sub command (https://github.com/denoland/deno/issues/15960). Found: {}", npm_specifier)
  } else {
    Ok(())
  }
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
