// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
use deno_terminal::colors;

use crate::args::CheckFlags;
use crate::args::Flags;
use crate::args::TypeCheckModeExt;
use crate::factory::CliFactory;
use crate::util::file_watcher;

pub async fn check(
  flags: Arc<Flags>,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  if let Some(watch_flags) = &flags.watch {
    // The native check path doesn't yet wire the resolved graph into the file
    // watcher, so it runs once and never re-checks. Warn rather than appear to
    // watch. See https://github.com/denoland/deno/issues/36089.
    log::warn!(
      "{} `deno check --watch` does not re-check on file changes yet; it runs once. See https://github.com/denoland/deno/issues/36089",
      colors::yellow("Warning")
    );
    let no_clear_screen = watch_flags.no_clear_screen;
    file_watcher::watch_func(
      flags,
      file_watcher::PrintConfig::new("Check", !no_clear_screen),
      move |flags, watcher_communicator, changed_paths| {
        let check_flags = check_flags.clone();
        watcher_communicator.show_path_changed(changed_paths);
        Ok(async move { native_check(flags, check_flags).await })
      },
    )
    .await
  } else {
    native_check(flags, check_flags).await
  }
}

/// Type-check the project with the native TypeScript compiler.
///
/// `deno check` generates a stock `tsconfig.json` and materializes the types of
/// the project's dependencies (the same work `deno sync-types` performs), then
/// runs the pinned native `tsc` against it and remaps the compiler's
/// diagnostics back onto the original module specifiers.
async fn native_check(
  flags: Arc<Flags>,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  if check_flags.doc || check_flags.doc_only {
    // Doc snippet extraction was handled by Deno 2.x's forked tsc; the native
    // compiler does not type-check markdown/JSDoc snippets yet.
    log::warn!(
      "{} --doc/--doc-only is not yet supported by the native type checker and will be ignored",
      colors::yellow("Warning")
    );
  }

  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;

  // Build the module graph over the requested roots (or the whole project).
  // Deno owns resolution: this drives deno's own graph diagnostics (missing
  // modules + hints) and the incremental type-check cache, so we can skip the
  // external compiler entirely when nothing it sees has changed.
  // Resolve the requested roots the same way `deno check` always has:
  // globs are expanded, workspace `exclude` is applied, and
  // `include_ignored_specified: false` means an explicitly-passed excluded file
  // is skipped rather than force-checked. An empty result is not an error - it
  // just means there's nothing to check (e.g. every match was excluded), which
  // deno reports as a warning and a clean exit.
  let graph_container = factory.main_module_graph_container().await?;
  let mut roots = graph_container.collect_specifiers(
    &check_flags.files,
    crate::graph_container::CollectSpecifiersOptions {
      include_ignored_specified: false,
    },
  )?;
  // `collect_specifiers` can yield the same workspace-member file more than
  // once (resolved both by its explicit path and via the member's package
  // `exports`), which would otherwise duplicate the `Check <specifier>` line
  // and the tsc `files` entry. Deduplicate while preserving order.
  {
    let mut seen = std::collections::HashSet::new();
    roots.retain(|s| seen.insert(s.clone()));
  }
  if roots.is_empty() {
    log::warn!("{} No matching files found.", colors::yellow("Warning"));
    return Ok(());
  }
  let graph_kind = cli_options.type_check_mode().as_graph_kind();
  let imports = factory
    .module_graph_builder()
    .await?
    .maybe_resolve_ts_config_imports(graph_kind);
  let graph = factory
    .module_graph_creator()
    .await?
    .create_graph_with_options(crate::graph_util::CreateGraphOptions {
      is_dynamic: false,
      graph_kind,
      roots: roots.clone(),
      imports,
      loader: None,
      npm_caching: cli_options.default_npm_caching_strategy(),
    })
    .await?;

  // Surface deno's own graph-resolution errors before handing off to tsc:
  // unsupported import attributes (e.g. `.css` without --unstable-raw-imports),
  // `compilerOptions.paths` that resolve to nothing, integrity/npm-resolution
  // failures, and invalid specifiers. Missing-module errors are deliberately
  // *not* surfaced here (`will_type_check` defers them to tsc, which reports
  // them as TS2307), so a missing import is never double-reported. Note: the
  // richer `deno add` hint Deno 2.x's forked tsc attached to import-level
  // missing modules is not re-added on top of tsc's TS2307 yet; restoring it
  // additively is a follow-up. `allow_unknown_media_types: true` matches
  // `deno check`, letting tsc handle unknown types instead of erroring.
  factory
    .module_graph_builder()
    .await?
    .graph_roots_valid(&graph, &roots, true, false)?;

  // Enforce the lockfile now that the graph has resolved the project's deps:
  // under `--frozen` this errors if the lockfile is out of date, otherwise it
  // writes the updated lockfile. Deno 2.x's forked tsc did this via the
  // module loader; native check builds the graph itself, so do it here - before
  // materializing types and spawning tsc, so `--frozen` fails fast.
  if let Some(lockfile) = factory.maybe_lockfile().await? {
    lockfile.write_if_changed()?;
  }

  // The graph-aware native check pipeline (cache decision, sync-types, tsc
  // download, spawn, diagnostic remapping, hash record) lives on
  // `TypeChecker::check_native`, so run/test/cache can share it. This walks the
  // graph exactly as Deno 2.x's forked tsc did for the incremental cache; deno's
  // own graph diagnostics are not merged additively yet (see
  // `walk_graph_for_native_check`).
  let type_checker = factory.type_checker().await?;
  type_checker
    .check_native(
      &graph,
      &roots,
      crate::type_checker::CheckOptions {
        build_fast_check_graph: false,
        lib: cli_options.ts_type_lib_window(),
        reload: cli_options.reload_flag(),
        type_check_mode: cli_options.type_check_mode(),
      },
    )
    .await?;

  Ok(())
}
