// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_graph::Module;
use deno_terminal::colors;

use crate::args::BundleFlags;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::TsConfigType;
use crate::factory::CliFactory;
use crate::factory::CliFactoryBuilder;
use crate::graph_util::error_for_any_npm_specifier;
use crate::util;
use crate::util::display;

pub async fn bundle(
  flags: Flags,
  bundle_flags: BundleFlags,
) -> Result<(), AnyError> {
  log::info!(
    "{}",
    colors::yellow("⚠️ Warning: `deno bundle` is deprecated and will be removed in Deno 2.0.\nUse an alternative bundler like \"deno_emit\", \"esbuild\" or \"rollup\" instead."),
  );

  if let Some(watch_flags) = &bundle_flags.watch {
    util::file_watcher::watch_func(
      flags,
      util::file_watcher::PrintConfig::new(
        "Bundle",
        !watch_flags.no_clear_screen,
      ),
      move |flags, watcher_communicator, _changed_paths| {
        let bundle_flags = bundle_flags.clone();
        Ok(async move {
          let factory = CliFactoryBuilder::new()
            .build_from_flags_for_watcher(flags, watcher_communicator.clone())
            .await?;
          let cli_options = factory.cli_options();
          let _ = watcher_communicator.watch_paths(cli_options.watch_paths());
          bundle_action(factory, &bundle_flags).await?;

          Ok(())
        })
      },
    )
    .await?;
  } else {
    let factory = CliFactory::from_flags(flags).await?;
    bundle_action(factory, &bundle_flags).await?;
  }

  Ok(())
}

async fn bundle_action(
  factory: CliFactory,
  bundle_flags: &BundleFlags,
) -> Result<(), AnyError> {
  let cli_options = factory.cli_options();
  let module_specifier = cli_options.resolve_main_module()?;
  log::debug!(">>>>> bundle START");
  let module_graph_creator = factory.module_graph_creator().await?;
  let cli_options = factory.cli_options();

  let graph = module_graph_creator
    .create_graph_and_maybe_check(vec![module_specifier.clone()])
    .await?;

  let mut paths_to_watch: Vec<PathBuf> = graph
    .specifiers()
    .filter_map(|(_, r)| {
      r.ok().and_then(|module| match module {
        Module::Js(m) => m.specifier.to_file_path().ok(),
        Module::Json(m) => m.specifier.to_file_path().ok(),
        // nothing to watch
        Module::Node(_) | Module::Npm(_) | Module::External(_) => None,
      })
    })
    .collect();

  if let Ok(Some(import_map_path)) = cli_options
    .resolve_specified_import_map_specifier()
    .map(|ms| ms.and_then(|ref s| s.to_file_path().ok()))
  {
    paths_to_watch.push(import_map_path);
  }

  // at the moment, we don't support npm specifiers in deno bundle, so show an error
  error_for_any_npm_specifier(&graph)?;

  let bundle_output = bundle_module_graph(graph.as_ref(), cli_options)?;
  log::debug!(">>>>> bundle END");
  let out_file = &bundle_flags.out_file;

  if let Some(out_file) = out_file {
    let out_file = cli_options.initial_cwd().join(out_file);
    let output_bytes = bundle_output.code.as_bytes();
    let output_len = output_bytes.len();
    util::fs::write_file(&out_file, output_bytes, 0o644)?;
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

fn bundle_module_graph(
  graph: &deno_graph::ModuleGraph,
  cli_options: &CliOptions,
) -> Result<deno_emit::BundleEmit, AnyError> {
  log::info!("{} {}", colors::green("Bundle"), graph.roots[0]);

  let ts_config_result =
    cli_options.resolve_ts_config_for_emit(TsConfigType::Bundle)?;
  if !cli_options.type_check_mode().is_true() {
    if let Some(ignored_options) = ts_config_result.maybe_ignored_options {
      log::warn!("{}", ignored_options);
    }
  }

  deno_emit::bundle_graph(
    graph,
    deno_emit::BundleOptions {
      minify: false,
      bundle_type: deno_emit::BundleType::Module,
      emit_options: crate::args::ts_config_to_emit_options(
        ts_config_result.ts_config,
      ),
      emit_ignore_directives: true,
    },
  )
}
