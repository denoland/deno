// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::resolve_url_or_path;
use deno_runtime::colors;

use crate::args::BundleFlags;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::TsConfigType;
use crate::args::TypeCheckMode;
use crate::graph_util::create_graph_and_maybe_check;
use crate::graph_util::error_for_any_npm_specifier;
use crate::proc_state::ProcState;
use crate::util;
use crate::util::display;
use crate::util::file_watcher::ResolutionResult;

pub async fn bundle(
  flags: Flags,
  bundle_flags: BundleFlags,
) -> Result<(), AnyError> {
  let cli_options = Arc::new(CliOptions::from_flags(flags)?);
  let resolver = |_| {
    let cli_options = cli_options.clone();
    let source_file1 = bundle_flags.source_file.clone();
    let source_file2 = bundle_flags.source_file.clone();
    async move {
      let module_specifier = resolve_url_or_path(&source_file1)?;

      log::debug!(">>>>> bundle START");
      let ps = ProcState::from_options(cli_options).await?;
      let graph = create_graph_and_maybe_check(module_specifier, &ps).await?;

      let mut paths_to_watch: Vec<PathBuf> = graph
        .specifiers()
        .filter_map(|(_, r)| {
          r.as_ref().ok().and_then(|(s, _, _)| s.to_file_path().ok())
        })
        .collect();

      if let Ok(Some(import_map_path)) = ps
        .options
        .resolve_import_map_specifier()
        .map(|ms| ms.and_then(|ref s| s.to_file_path().ok()))
      {
        paths_to_watch.push(import_map_path);
      }

      Ok((paths_to_watch, graph, ps))
    }
    .map(move |result| match result {
      Ok((paths_to_watch, graph, ps)) => ResolutionResult::Restart {
        paths_to_watch,
        result: Ok((ps, graph)),
      },
      Err(e) => ResolutionResult::Restart {
        paths_to_watch: vec![PathBuf::from(source_file2)],
        result: Err(e),
      },
    })
  };

  let operation = |(ps, graph): (ProcState, Arc<deno_graph::ModuleGraph>)| {
    let out_file = bundle_flags.out_file.clone();
    async move {
      // at the moment, we don't support npm specifiers in deno bundle, so show an error
      error_for_any_npm_specifier(&graph)?;

      let bundle_output = bundle_module_graph(graph.as_ref(), &ps)?;
      log::debug!(">>>>> bundle END");

      if let Some(out_file) = out_file.as_ref() {
        let output_bytes = bundle_output.code.as_bytes();
        let output_len = output_bytes.len();
        util::fs::write_file(out_file, output_bytes, 0o644)?;
        log::info!(
          "{} {:?} ({})",
          colors::green("Emit"),
          out_file,
          colors::gray(display::human_size(output_len as f64))
        );
        if let Some(bundle_map) = bundle_output.maybe_map {
          let map_bytes = bundle_map.as_bytes();
          let map_len = map_bytes.len();
          let ext = if let Some(curr_ext) = out_file.extension() {
            format!("{}.map", curr_ext.to_string_lossy())
          } else {
            "map".to_string()
          };
          let map_out_file = out_file.with_extension(ext);
          util::fs::write_file(&map_out_file, map_bytes, 0o644)?;
          log::info!(
            "{} {:?} ({})",
            colors::green("Emit"),
            map_out_file,
            colors::gray(display::human_size(map_len as f64))
          );
        }
      } else {
        println!("{}", bundle_output.code);
      }

      Ok(())
    }
  };

  if cli_options.watch_paths().is_some() {
    util::file_watcher::watch_func(
      resolver,
      operation,
      util::file_watcher::PrintConfig {
        job_name: "Bundle".to_string(),
        clear_screen: !cli_options.no_clear_screen(),
      },
    )
    .await?;
  } else {
    let module_graph =
      if let ResolutionResult::Restart { result, .. } = resolver(None).await {
        result?
      } else {
        unreachable!();
      };
    operation(module_graph).await?;
  }

  Ok(())
}

fn bundle_module_graph(
  graph: &deno_graph::ModuleGraph,
  ps: &ProcState,
) -> Result<deno_emit::BundleEmit, AnyError> {
  log::info!("{} {}", colors::green("Bundle"), graph.roots[0].0);

  let ts_config_result = ps
    .options
    .resolve_ts_config_for_emit(TsConfigType::Bundle)?;
  if ps.options.type_check_mode() == TypeCheckMode::None {
    if let Some(ignored_options) = ts_config_result.maybe_ignored_options {
      log::warn!("{}", ignored_options);
    }
  }

  let mut output = deno_emit::bundle_graph(
    graph,
    deno_emit::BundleOptions {
      bundle_type: deno_emit::BundleType::Module,
      emit_options: ts_config_result.ts_config.into(),
      emit_ignore_directives: true,
    },
  )?;

  // todo(https://github.com/denoland/deno_emit/issues/85): move to deno_emit
  if let Some(shebang) = shebang_file(graph) {
    output.code = format!("{}\n{}", shebang, output.code);
  }

  Ok(output)
}

fn shebang_file(graph: &deno_graph::ModuleGraph) -> Option<String> {
  let source = graph.get(&graph.roots[0].0)?.maybe_source.as_ref()?;
  let first_line = source.lines().next()?;
  if first_line.starts_with("#!") {
    Some(first_line.to_string())
  } else {
    None
  }
}
