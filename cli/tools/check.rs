// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
use deno_terminal::colors;

use crate::args::CheckFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::graph_container::CheckSpecifiersOptions;
use crate::graph_container::CollectSpecifiersOptions;
use crate::util::extract;
use crate::util::file_watcher;

/// Experimental env gate: when set, `deno check` type-checks with native
/// `tsc` (the `typescript@7` npm package, which ships a platform-native
/// binary) against a `deno sync-types`-generated `tsconfig.json`, instead of
/// the in-isolate JS TypeScript. This is the target architecture for the
/// TypeScript un-fork; it's gated while the diagnostic mapping (tsc reports
/// paths into the generated `.deno/`/`node_modules` mirrors, not the original
/// specifiers) and the default-on rollout are worked out.
const NATIVE_CHECK_ENV_VAR: &str = "DENO_UNSTABLE_NATIVE_CHECK";

/// The `typescript@7` bin spec run to perform the native type-check. `@7`
/// pins to the native (Go/tsgo) compiler line; `6.x` is the last JS one.
const NATIVE_TSC_BIN_SPECIFIER: &str = "npm:typescript@7/tsc";

pub async fn check(
  flags: Arc<Flags>,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  if std::env::var_os(NATIVE_CHECK_ENV_VAR).is_some() {
    return check_native(flags, check_flags).await;
  }
  if let Some(watch_flags) = &flags.watch {
    let no_clear_screen = watch_flags.no_clear_screen;
    file_watcher::watch_func(
      flags,
      file_watcher::PrintConfig::new("Check", !no_clear_screen),
      move |flags, watcher_communicator, changed_paths| {
        let check_flags = check_flags.clone();
        watcher_communicator.show_path_changed(changed_paths);
        Ok(async move {
          let factory = CliFactory::from_flags_for_watcher(
            flags,
            watcher_communicator.clone(),
          );
          check_with_factory(&factory, check_flags).await
        })
      },
    )
    .await
  } else {
    let factory = CliFactory::from_flags(flags);
    check_with_factory(&factory, check_flags).await
  }
}

async fn check_with_factory(
  factory: &CliFactory,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  let main_graph_container = factory.main_module_graph_container().await?;

  let specifiers = main_graph_container.collect_specifiers(
    &check_flags.files,
    CollectSpecifiersOptions {
      include_ignored_specified: false,
    },
  )?;
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
    .check_specifiers(
      &specifiers_for_typecheck,
      CheckSpecifiersOptions {
        allow_unknown_media_types: true,
        ..Default::default()
      },
    )
    .await
}

/// Type-check the project with native `tsc` instead of the in-isolate JS
/// TypeScript. Runs `deno sync-types` to materialize dependency types and emit
/// a stock `tsconfig.json`, then invokes `typescript@7`'s native `tsc` against
/// it. See [`NATIVE_CHECK_ENV_VAR`].
async fn check_native(
  flags: Arc<Flags>,
  _check_flags: CheckFlags,
) -> Result<(), AnyError> {
  // Materialize the dependency types and generate the tsconfig.json so stock
  // TypeScript can resolve the project's jsr:/http(s): imports. This is exactly
  // the work `deno sync-types` performs.
  crate::tools::installer::sync_types_command(flags.clone()).await?;

  // Locate the generated root tsconfig.json at the workspace root (not the cwd,
  // so running from a subdirectory still checks the whole project).
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let project_root = cli_options
    .workspace()
    .root_dir_url()
    .to_file_path()
    .map_err(|_| {
      deno_core::anyhow::anyhow!("workspace root is not a local directory")
    })?;
  let tsconfig_path = project_root.join("tsconfig.json");

  // Run native `tsc`. `typescript@7` ships the compiler as a platform-native
  // binary via optionalDependencies; rather than reimplement npm bin
  // resolution and native-binary dispatch here, reuse deno's own runner by
  // shelling out to `deno run` on the bin specifier. deno caches only the
  // host-platform binary and dispatches to it. The generated tsconfig already
  // sets `noEmit`; we pass it explicitly too so a stray CLI flag can't cause
  // emit.
  let deno_exe = std::env::current_exe()?;
  log::info!(
    "{} with native tsc ({})",
    colors::green("Checking"),
    NATIVE_TSC_BIN_SPECIFIER,
  );
  let status = tokio::process::Command::new(deno_exe)
    .arg("run")
    .arg("--allow-all")
    .arg(NATIVE_TSC_BIN_SPECIFIER)
    .arg("--project")
    .arg(&tsconfig_path)
    .arg("--noEmit")
    .arg("--pretty")
    .current_dir(&project_root)
    .status()
    .await?;

  if !status.success() {
    // tsc has already printed the diagnostics; surface a non-zero exit.
    deno_core::anyhow::bail!("Type checking failed.");
  }
  Ok(())
}
