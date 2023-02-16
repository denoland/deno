// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::Lockfile;
use crate::args::TsConfigType;
use crate::args::TypeCheckMode;
use crate::cache;
use crate::cache::TypeCheckCache;
use crate::colors;
use crate::errors::get_error_class_name;
use crate::npm::resolve_graph_npm_info;
use crate::proc_state::ProcState;
use crate::resolver::CliGraphResolver;
use crate::tools::check;

use deno_core::anyhow::bail;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ResolutionError;
use deno_graph::SpecifierError;
use deno_runtime::permissions::PermissionsContainer;
use import_map::ImportMapError;
use std::sync::Arc;

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
  let maybe_package_json_deps = ps.options.maybe_package_json_deps()?;
  let cli_resolver = CliGraphResolver::new(
    ps.options.to_maybe_jsx_import_source_config(),
    ps.maybe_import_map.clone(),
    maybe_package_json_deps,
  );
  let graph_resolver = cli_resolver.as_graph_resolver();
  let analyzer = ps.parsed_source_cache.as_analyzer();
  let mut graph = ModuleGraph::default();
  graph
    .build(
      vec![root],
      &mut cache,
      deno_graph::BuildOptions {
        is_dynamic: false,
        imports: maybe_imports,
        resolver: Some(graph_resolver),
        module_analyzer: Some(&*analyzer),
        reporter: None,
      },
    )
    .await;
  graph_valid_with_cli_options(&graph, &graph.roots, &ps.options)?;
  let graph = Arc::new(graph);
  let npm_graph_info = resolve_graph_npm_info(&graph);
  ps.npm_resolver
    .add_package_reqs(npm_graph_info.package_reqs)
    .await?;
  if let Some(lockfile) = &ps.lockfile {
    graph_lock_or_exit(&graph, &mut lockfile.lock());
  }

  if ps.options.type_check_mode() != TypeCheckMode::None {
    // node built-in specifiers use the @types/node package to determine
    // types, so inject that now after the lockfile has been written
    if npm_graph_info.has_node_builtin_specifier {
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
        has_node_builtin_specifier: npm_graph_info.has_node_builtin_specifier,
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
