// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::GraphKind;
use deno_path_util::url_from_directory_path;

use deno_bundler::analyze::analyze_graph;
use deno_bundler::chunk::build_chunk_graph;
use deno_bundler::chunk::ChunkType;
use deno_bundler::config::EnvironmentId;
use deno_bundler::emit::emit_dev_chunk;
use deno_bundler::graph_builder::build_bundler_graph;
use deno_bundler::transpile::transpile_graph;

use crate::args::BuildFlags;
use crate::args::Flags;
use crate::colors;
use crate::factory::CliFactory;
use deno_npm_installer::graph::NpmCachingStrategy;

pub async fn build(
  flags: Arc<Flags>,
  _build_flags: BuildFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;

  let deno_json = cli_options
    .start_dir
    .member_or_root_deno_json()
    .ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "No deno.json found. \"deno build\" requires a deno.json with a \"build\" section."
      )
    })?;

  let build_config = deno_json
    .to_build_config()
    .map_err(|e| {
      deno_core::anyhow::anyhow!("Failed to parse build config: {}", e)
    })?
    .ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "No \"build\" configuration found in deno.json. Add a \"build\" section with environments."
      )
    })?;

  let root_dir = deno_json
    .specifier
    .to_file_path()
    .map(|p| p.parent().unwrap().to_path_buf())
    .unwrap_or_else(|_| std::env::current_dir().unwrap());

  let env_count = build_config.environments.len();
  log::warn!(
    "{} production build ({} environment{})",
    colors::green("Starting"),
    env_count,
    if env_count == 1 { "" } else { "s" },
  );

  let graph_creator = factory.module_graph_creator().await?.clone();
  let root_url = url_from_directory_path(&root_dir)?;

  let mut env_id_counter = 0u32;
  let mut total_chunks = 0usize;
  let mut total_modules = 0usize;

  for (name, env_config) in &build_config.environments {
    let env_id = EnvironmentId::new(env_id_counter);
    env_id_counter += 1;

    let entries: Vec<ModuleSpecifier> = env_config
      .entries
      .iter()
      .map(|entry| {
        root_url.join(entry).unwrap_or_else(|_| {
          ModuleSpecifier::parse(&format!("file:///{}", entry)).unwrap()
        })
      })
      .collect();

    log::warn!(
      "\n  {} {} ({} entries)...",
      colors::green("Building"),
      name,
      entries.len(),
    );

    // Build deno_graph.
    let deno_module_graph = graph_creator
      .create_graph(GraphKind::All, entries.clone(), NpmCachingStrategy::Eager)
      .await?;

    // Convert, transpile, and analyze.
    let mut bundler_graph =
      build_bundler_graph(&deno_module_graph, env_id, &entries);
    transpile_graph(&mut bundler_graph);
    analyze_graph(&mut bundler_graph);

    let module_count = bundler_graph.len();
    total_modules += module_count;

    // Build chunks.
    let chunk_graph = build_chunk_graph(&bundler_graph);
    let chunk_count = chunk_graph.len();
    total_chunks += chunk_count;

    log::warn!(
      "  {} {} → {} modules, {} chunks",
      colors::green("Built"),
      name,
      module_count,
      chunk_count,
    );

    // Determine output directory.
    let output_dir = if let Some(output) = &env_config.output {
      root_dir.join(output)
    } else {
      root_dir.join("dist").join(name)
    };

    // Create output directory.
    std::fs::create_dir_all(&output_dir).map_err(|e| {
      deno_core::anyhow::anyhow!(
        "Failed to create output directory {}: {}",
        output_dir.display(),
        e
      )
    })?;

    // Emit chunks.
    // TODO: Use production emit with scope hoisting + minification.
    // For now, use dev emit format.
    for chunk in chunk_graph.chunks() {
      let output = emit_dev_chunk(chunk, &bundler_graph, &chunk_graph);

      let filename = match chunk.chunk_type {
        ChunkType::Entry => {
          if let Some(entry) = &chunk.entry {
            let name = entry
              .path_segments()
              .and_then(|s| s.last())
              .unwrap_or("entry");
            let name = name
              .strip_suffix(".ts")
              .or_else(|| name.strip_suffix(".tsx"))
              .or_else(|| name.strip_suffix(".jsx"))
              .or_else(|| name.strip_suffix(".js"))
              .unwrap_or(name);
            format!("{}.js", name)
          } else {
            format!("chunk_{}.js", chunk.id.0)
          }
        }
        ChunkType::DynamicImport => {
          if let Some(entry) = &chunk.entry {
            let name = entry
              .path_segments()
              .and_then(|s| s.last())
              .unwrap_or("dynamic");
            let name = name
              .strip_suffix(".ts")
              .or_else(|| name.strip_suffix(".tsx"))
              .or_else(|| name.strip_suffix(".jsx"))
              .or_else(|| name.strip_suffix(".js"))
              .unwrap_or(name);
            format!("{}.js", name)
          } else {
            format!("chunk_{}.js", chunk.id.0)
          }
        }
        ChunkType::Shared => format!("shared_{}.js", chunk.id.0),
        ChunkType::Asset => format!("asset_{}", chunk.id.0),
      };

      let out_path = output_dir.join(&filename);
      std::fs::write(&out_path, &output.code).map_err(|e| {
        deno_core::anyhow::anyhow!(
          "Failed to write {}: {}",
          out_path.display(),
          e
        )
      })?;

      log::warn!(
        "    {} {} ({} bytes)",
        colors::green("→"),
        filename,
        output.code.len(),
      );
    }

    log::warn!(
      "  {} {}",
      colors::green("Output:"),
      output_dir.display(),
    );
  }

  log::warn!(
    "\n{} {} modules → {} chunks",
    colors::green("Done!"),
    total_modules,
    total_chunks,
  );

  Ok(())
}
