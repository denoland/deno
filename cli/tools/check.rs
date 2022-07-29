// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::CheckFlags;
use crate::args::Flags;
use crate::file_watcher;
use crate::file_watcher::ResolutionResult;
use crate::proc_state::ProcState;

use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::resolve_url_or_path;
use deno_core::ModuleSpecifier;
use deno_runtime::permissions::Permissions;
use std::path::PathBuf;

pub async fn run_check_with_watch(
  flags: Flags,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  let ps = ProcState::build(flags).await?;
  let files = check_flags.files;
  let paths_to_watch: Vec<_> = files.iter().map(PathBuf::from).collect();

  // Check first time.
  for file in files {
    let specifier = resolve_url_or_path(&file)?;
    do_check(specifier, &ps, true).await?;
  }

  // Check Watch
  let resolver = |changed: Option<Vec<PathBuf>>| {
    let paths_to_watch = paths_to_watch.clone();
    let paths_to_watch_clone = paths_to_watch.clone();

    async move {
      let paths_to_watch = paths_to_watch_clone;
      let mut modules_to_reload = Vec::new();

      if let Some(changed) = &changed {
        for path in changed
          .iter()
          .filter_map(|path| resolve_url_or_path(&path.to_string_lossy()).ok())
        {
          modules_to_reload.push(path);
        }
      }

      Ok((paths_to_watch, modules_to_reload))
    }
    .map(move |result| match result {
      Ok((paths_to_watch, modules_to_reload)) => ResolutionResult::Restart {
        paths_to_watch,
        result: Ok(modules_to_reload),
      },
      Err(e) => ResolutionResult::Restart {
        paths_to_watch,
        result: Err(e),
      },
    })
  };

  let operation = |modules_to_reload: Vec<ModuleSpecifier>| {
    let ps = ps.clone();

    async move {
      for check_module in modules_to_reload {
        do_check(check_module, &ps, true).await?;
      }
      Ok(())
    }
  };

  let cli_options = ps.options.clone();

  file_watcher::watch_func(
    resolver,
    operation,
    file_watcher::PrintConfig {
      job_name: "Check".to_string(),
      clear_screen: !cli_options.no_clear_screen(),
    },
  )
  .await?;

  Ok(())
}

pub async fn do_check(
  check_module: ModuleSpecifier,
  ps: &ProcState,
  reload_on_watch: bool,
) -> Result<(), AnyError> {
  ps.prepare_module_load(
    vec![check_module],
    false,
    ps.options.ts_type_lib_window(),
    Permissions::allow_all(),
    Permissions::allow_all(),
    reload_on_watch,
  )
  .await?;

  Ok(())
}
