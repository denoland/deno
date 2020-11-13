// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::file_watcher;
use crate::flags::Flags;
use crate::fs_util;
use crate::info;
use crate::module_graph;
use crate::program_state::ProgramState;

use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::ModuleSpecifier;
use std::path::PathBuf;

pub fn bundle_module_graph(
  module_graph: module_graph::Graph,
  flags: Flags,
  debug: bool,
) -> Result<String, AnyError> {
  let (bundle, stats, maybe_ignored_options) =
    module_graph.bundle(module_graph::BundleOptions {
      debug,
      maybe_config_path: flags.config_path,
    })?;
  match maybe_ignored_options {
    Some(ignored_options) if flags.no_check => {
      eprintln!("{}", ignored_options);
    }
    _ => {}
  }
  debug!("{}", stats);
  Ok(bundle)
}

/// Returns a closure which returns the module graph of
/// the given source file. The closure can be passed to file_watcher.
pub fn get_module_resolver(
  flags: &Flags,
  source_file: &str,
) -> impl Fn() -> file_watcher::FileWatcherFuture<
  file_watcher::ModuleResolutionResult<module_graph::Graph>,
> {
  let flags = flags.clone();
  let source_file = source_file.to_string();
  let debug = flags.log_level == Some(log::Level::Debug);
  move || {
    let flags = flags.clone();
    let source_file1 = source_file.clone();
    let source_file2 = source_file.clone();
    async move {
      let module_specifier =
        ModuleSpecifier::resolve_url_or_path(&source_file1)?;

      debug!(">>>>> bundle START");
      let program_state = ProgramState::new(flags.clone())?;

      info!(
        "{} {}",
        colors::green("Bundle"),
        module_specifier.to_string()
      );

      let module_graph = module_graph::create_module_graph_and_maybe_check(
        module_specifier,
        program_state.clone(),
        debug,
      )
      .await?;

      let mut paths_to_watch: Vec<PathBuf> = module_graph
        .get_modules()
        .iter()
        .filter_map(|specifier| specifier.as_url().to_file_path().ok())
        .collect();

      if let Some(import_map) = program_state.flags.import_map_path.as_ref() {
        paths_to_watch
          .push(fs_util::resolve_from_cwd(std::path::Path::new(import_map))?);
      }

      Ok((paths_to_watch, module_graph))
    }
    .map(move |result| match result {
      Ok((paths_to_watch, module_graph)) => {
        file_watcher::ModuleResolutionResult::Success {
          paths_to_watch,
          module_info: module_graph,
        }
      }
      Err(e) => file_watcher::ModuleResolutionResult::Fail {
        source_path: PathBuf::from(source_file2),
        error: e,
      },
    })
    .boxed_local()
  }
}

/// Returns a closure which takes the module graph and performs
/// the bundling operation. The closure can be passed to file_watcher.
pub fn get_operation(
  flags: &Flags,
  out_file: &Option<PathBuf>,
) -> impl Fn(
  module_graph::Graph,
) -> file_watcher::FileWatcherFuture<Result<(), AnyError>> {
  let flags = flags.clone();
  let out_file = out_file.clone();
  move |module_graph: module_graph::Graph| {
    let flags = flags.clone();
    let out_file = out_file.clone();
    let debug = flags.log_level == Some(log::Level::Debug);
    async move {
      let output = bundle_module_graph(module_graph, flags, debug)?;

      debug!(">>>>> bundle END");

      if let Some(out_file) = out_file.as_ref() {
        let output_bytes = output.as_bytes();
        let output_len = output_bytes.len();
        fs_util::write_file(out_file, output_bytes, 0o644)?;
        info!(
          "{} {:?} ({})",
          colors::green("Emit"),
          out_file,
          colors::gray(&info::human_size(output_len as f64))
        );
      } else {
        println!("{}", output);
      }

      Ok(())
    }
    .boxed_local()
  }
}
