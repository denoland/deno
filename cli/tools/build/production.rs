// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::GraphKind;
use deno_path_util::url_from_directory_path;

use deno_bundler::analyze::analyze_graph;
use deno_bundler::analyze::tree_shake_graph;
use deno_bundler::asset_discovery::discover_assets;
use deno_bundler::chunk::build_chunk_graph;
use deno_bundler::config::EnvironmentId;
use deno_bundler::emit::cross_chunk::compute_cross_chunk_bindings;
use deno_bundler::emit::emit_production_chunk;
use deno_bundler::graph_builder::build_bundler_graph;
use deno_bundler::plugin::create_default_plugin_driver;
use deno_bundler::plugin::create_plugin_driver;
use deno_bundler::process::transform_modules;
use deno_bundler::transform_pipeline::transform_graph;
use deno_bundler::transform_pipeline::TransformOptions;

use crate::args::BuildFlags;
use crate::args::Flags;
use crate::colors;
use crate::factory::CliFactory;
use deno_npm_installer::graph::NpmCachingStrategy;

use super::plugin_host;

pub async fn build(
  flags: Arc<Flags>,
  build_flags: BuildFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;

  // Load .env file variables as defines for process.env.* replacement.
  let env_defines = load_env_defines(build_flags.env_file.as_ref());

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

  // Resolve and load JS plugins if configured.
  let plugin_driver = if !build_config.plugins.is_empty() {
    let specifiers = plugin_host::resolve_plugin_specifiers(&build_config.plugins)?;
    for spec in &specifiers {
      log::warn!("  {} {}", colors::cyan("plugin"), spec);
    }
    let proxy = plugin_host::create_and_load_plugins(specifiers).await?;
    let proxy = Arc::new(proxy);
    let bridge = plugin_host::JsPluginBridge::new(
      proxy,
      tokio::runtime::Handle::current(),
    );
    create_plugin_driver(vec![Box::new(bridge)])
  } else {
    create_default_plugin_driver()
  };

  let graph_creator = factory.module_graph_creator().await?.clone();
  let root_url = url_from_directory_path(&root_dir)?;

  let mut env_id_counter = 0u32;
  let mut total_chunks = 0usize;
  let mut total_modules = 0usize;

  // Default production defines.
  let mut defines = HashMap::new();
  defines.insert(
    "process.env.NODE_ENV".to_string(),
    "\"production\"".to_string(),
  );
  // Merge .env file defines (don't override explicit defines like NODE_ENV).
  for (key, value) in &env_defines {
    defines.entry(key.clone()).or_insert_with(|| value.clone());
  }

  let transform_options = TransformOptions {
    define: defines,
    dead_code_elimination: true,
    convert_to_var: false,
    production: true,
  };

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

    // Convert, transform (includes transpilation), discover assets, and analyze.
    let mut bundler_graph =
      build_bundler_graph(&deno_module_graph, env_id, &entries);
    transform_modules(&mut bundler_graph, &plugin_driver);
    transform_graph(&mut bundler_graph, &transform_options);
    discover_assets(&mut bundler_graph);
    analyze_graph(&mut bundler_graph);

    // Cross-module binding resolution and tree shaking.
    bundler_graph.resolve_cross_module_bindings();
    bundler_graph.compute_used_exports();
    tree_shake_graph(&mut bundler_graph);

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

    // Compute cross-chunk bindings and content-hashed filenames.
    let cross_chunk =
      compute_cross_chunk_bindings(&chunk_graph, &bundler_graph);

    // Emit chunks using production format.
    for chunk in chunk_graph.chunks() {
      let output = emit_production_chunk(
        chunk,
        &bundler_graph,
        &chunk_graph,
        Some(&cross_chunk),
      );

      let filename = output.filename.clone();

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

    // Copy asset files to output directory with content-hashed names.
    let assets_dir = output_dir.join("assets");
    let mut asset_count = 0usize;
    for module in bundler_graph.modules() {
      if !module.loader.is_asset() {
        continue;
      }
      if let Ok(src_path) = module.specifier.to_file_path() {
        if src_path.exists() {
          // Create assets subdirectory.
          if asset_count == 0 {
            let _ = std::fs::create_dir_all(&assets_dir);
          }

          // Content-hash the filename.
          let file_bytes = std::fs::read(&src_path).unwrap_or_default();
          let hash = {
            use std::hash::Hasher;
            let mut hasher =
              std::collections::hash_map::DefaultHasher::new();
            hasher.write(&file_bytes);
            format!("{:x}", hasher.finish())
          };
          let stem = src_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("asset");
          let ext = src_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("bin");
          let hashed_name =
            format!("{}-{}.{}", stem, &hash[..8], ext);
          let out_path = assets_dir.join(&hashed_name);

          if let Err(e) = std::fs::write(&out_path, &file_bytes) {
            log::warn!(
              "    {} failed to copy asset {}: {}",
              colors::red("!"),
              src_path.display(),
              e
            );
          } else {
            log::warn!(
              "    {} assets/{} ({} bytes)",
              colors::green("→"),
              hashed_name,
              file_bytes.len(),
            );
            asset_count += 1;
          }
        }
      }
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

/// Read .env files and return a map of `process.env.KEY` → `"\"value\""` defines.
fn load_env_defines(
  env_files: Option<&Vec<String>>,
) -> HashMap<String, String> {
  let mut defines = HashMap::new();
  let Some(env_files) = env_files else {
    return defines;
  };
  for file_path in env_files {
    let path = std::path::Path::new(file_path);
    let iter = match deno_dotenv::from_path_sanitized_iter_with_substitution(
      &sys_traits::impls::RealSys,
      path,
    ) {
      Ok(iter) => iter,
      Err(e) => {
        log::warn!(
          "  {} Failed to read {}: {}",
          colors::yellow("Warning"),
          file_path,
          e,
        );
        continue;
      }
    };
    for item in iter {
      match item {
        Ok((key, value)) => {
          let define_key = format!("process.env.{}", key);
          // First file wins (don't override).
          defines
            .entry(define_key)
            .or_insert_with(|| format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")));
        }
        Err(e) => {
          log::warn!(
            "  {} Error parsing {}: {:?}",
            colors::yellow("Warning"),
            file_path,
            e,
          );
          break;
        }
      }
    }
  }
  defines
}
