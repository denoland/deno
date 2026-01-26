// Copyright 2018-2026 the Deno authors. MIT license.

//! Vite-like bundler with JavaScript plugins for Deno.
//!
//! This module implements a universal pre-processor/virtual file system that
//! serves as a transformation layer for all Deno tooling. It uses deno_ast
//! for parsing and code generation, and supports JavaScript-based plugins
//! following a Vite/Rollup-like pattern.
//!
//! # Architecture
//!
//! The bundler has a two-layer architecture:
//!
//! 1. **Source Module Graph** (`source_graph.rs`): Tracks all source modules
//!    across multiple environments (server, browser). This is the input to
//!    the bundler.
//!
//! 2. **Chunk Graphs** (TODO: `chunk_graph.rs`): Per-environment bundled
//!    chunks. This is the output of the bundler.
//!
//! The plugin system (`plugins.rs`) runs in a separate thread with its own
//! V8 isolate, communicating with the main bundler via channels.
//!
//! # Key Use Cases
//!
//! - Transform `.svelte`, `.vue`, `.astro` files to JavaScript
//! - Enable `deno run`, `deno test`, `deno lint`, `deno check` for non-JS files
//! - Bundle applications for deployment
//!
//! # Plugin API
//!
//! Plugins are JavaScript modules with the following hooks:
//!
//! ```typescript
//! export default {
//!   name: "plugin-name",
//!   // File extensions this plugin handles
//!   extensions: [".svelte"],
//!   // Resolve a module specifier
//!   resolveId(source, importer, options) { ... },
//!   // Load a module's source code
//!   load(id) { ... },
//!   // Transform source code
//!   transform(code, id) { ... },
//! }
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;

use crate::args::Flags;
use crate::args::VbundleFlags;
use crate::factory::CliFactory;

pub mod chunk_graph;
pub mod emitter;
pub mod environment;
pub mod hmr_runtime;
pub mod hmr_server;
pub mod hmr_types;
pub mod import_analyzer;
pub mod plugins;
pub mod source_graph;
pub mod source_map;
pub mod splitter;
pub mod types;
pub mod vfs_lint_adapter;
pub mod vfs_module_loader;
pub mod vfs_tsc_adapter;
pub mod virtual_fs;

pub use chunk_graph::Chunk;
pub use chunk_graph::ChunkGraph;
pub use chunk_graph::ChunkId;
pub use emitter::ChunkEmitter;
pub use emitter::EmitterConfig;
pub use environment::BundleEnvironment;
pub use hmr_runtime::generate_hmr_runtime;
pub use hmr_runtime::generate_module_hmr_wrapper;
pub use hmr_server::HmrModuleGraph;
pub use hmr_server::HmrServer;
pub use hmr_server::SharedHmrGraph;
pub use hmr_types::HmrBoundary;
pub use hmr_types::HmrConfig;
pub use hmr_types::HmrEvent;
pub use hmr_types::HmrModuleInfo;
pub use hmr_types::HmrUpdatePayload;
pub use plugins::PluginHostProxy;
pub use plugins::PluginLogger;
pub use plugins::create_runner_and_load_plugins;
pub use source_graph::SharedSourceGraph;
pub use source_graph::SourceModule;
pub use source_map::Position;
pub use source_map::SourceMapCache;
pub use source_map::SourceRange;
pub use splitter::CodeSplitter;
pub use splitter::SplitterConfig;
pub use types::BuildConfig;
pub use types::TransformedModule;
pub use vfs_lint_adapter::LintDiagnostic;
pub use vfs_lint_adapter::LintSeverity;
pub use vfs_lint_adapter::VfsLintAdapter;
pub use vfs_module_loader::ErrorPositionMapper;
pub use vfs_module_loader::VfsLoaderConfig;
pub use vfs_module_loader::VfsModuleLoader;
pub use vfs_tsc_adapter::TsDiagnostic;
pub use vfs_tsc_adapter::TsSeverity;
pub use vfs_tsc_adapter::VfsTscAdapter;
pub use virtual_fs::BundlerVirtualFS;
pub use virtual_fs::VfsBuilder;
pub use virtual_fs::VfsConfig;

/// Main entry point for the vbundle command.
pub async fn vbundle(
  flags: Arc<Flags>,
  vbundle_flags: VbundleFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;

  // Parse entry points
  let entry_points: Vec<ModuleSpecifier> = vbundle_flags
    .files
    .include
    .iter()
    .map(|f| deno_path_util::resolve_url_or_path(f, cli_options.initial_cwd()))
    .collect::<Result<Vec<_>, _>>()?;

  if entry_points.is_empty() {
    log::error!("No entry points specified");
    return Err(deno_core::anyhow::anyhow!("No entry points specified"));
  }

  // Parse plugin specifiers
  let plugin_specifiers: Vec<ModuleSpecifier> = vbundle_flags
    .plugins
    .iter()
    .map(|p| deno_path_util::resolve_url_or_path(p, cli_options.initial_cwd()))
    .collect::<Result<Vec<_>, _>>()?;

  // Create the build configuration
  let config = BuildConfig {
    entry_points: entry_points.clone(),
    out_dir: vbundle_flags
      .out_dir
      .map(PathBuf::from)
      .unwrap_or_else(|| PathBuf::from("dist")),
    sourcemap: !vbundle_flags.no_sourcemap,
    minify: vbundle_flags.minify,
    environments: parse_environments(&vbundle_flags.environments),
    plugins: plugin_specifiers.clone(),
  };

  log::info!(
    "Starting vbundle with {} entry points",
    config.entry_points.len()
  );

  // Create plugin host if there are plugins
  let maybe_plugin_host = if !plugin_specifiers.is_empty() {
    let logger = PluginLogger::new(|msg, is_error| {
      if is_error {
        eprintln!("{}", msg);
      } else {
        println!("{}", msg);
      }
    });
    let host =
      create_runner_and_load_plugins(plugin_specifiers, logger).await?;
    log::info!("Loaded {} plugins", host.get_plugins().len());
    Some(Arc::new(host))
  } else {
    None
  };

  // Create VFS with plugins
  let vfs = {
    let mut builder = VfsBuilder::new()
      .enable_cache(true)
      .source_maps(!vbundle_flags.no_sourcemap);

    if let Some(host) = &maybe_plugin_host {
      // Register extensions from plugins
      for plugin_info in host.get_plugins() {
        if !plugin_info.extensions.is_empty() {
          builder = builder.register_extensions(
            &plugin_info.name,
            plugin_info.extensions.clone(),
          );
        }
      }
      builder = builder.plugin_host(host.clone());
    }

    Arc::new(builder.build())
  };

  log::info!(
    "VFS initialized with {} extension handlers",
    vfs.extension_handler_count()
  );

  // Build the source module graph using VFS
  let graph = build_source_graph(&config, &vfs).await?;

  if graph.read().has_errors() {
    for error in graph.read().errors() {
      log::error!("Error loading {}: {}", error.specifier, error.message);
    }
    return Err(deno_core::anyhow::anyhow!("Failed to build module graph"));
  }

  log::info!(
    "Built source graph with {} modules",
    graph.read().module_count()
  );

  // Report VFS cache stats
  let stats = vfs.cache_stats();
  log::info!(
    "VFS cache: {} entries, {} source maps",
    stats.entries,
    stats.source_maps
  );

  // Phase 4: Code splitting and chunk generation
  let splitter_config = SplitterConfig::default();
  let splitter = CodeSplitter::new(&graph, splitter_config);

  // Parse build mode
  let build_mode = match vbundle_flags.mode.as_deref() {
    Some("production") => emitter::BuildMode::Production,
    _ => emitter::BuildMode::Development,
  };

  // Parse custom environment variables
  let mut env_vars = std::collections::HashMap::new();
  for define in &vbundle_flags.define {
    if let Some((key, value)) = define.split_once('=') {
      env_vars.insert(key.to_string(), value.to_string());
    }
  }

  // Configure HMR if enabled
  let hmr_config = if vbundle_flags.hmr {
    let mut hmr_config = hmr_types::HmrConfig::default();
    if let Some(port) = vbundle_flags.hmr_port {
      hmr_config = hmr_config.with_port(port);
    }
    Some(hmr_config)
  } else {
    None
  };

  let emitter_config = EmitterConfig {
    source_maps: config.sourcemap,
    minify: config.minify,
    out_dir: config.out_dir.clone(),
    mode: build_mode,
    env_vars,
    hmr: vbundle_flags.hmr,
    hmr_config,
  };

  // Generate chunks for each environment
  for env in &config.environments {
    log::info!("Splitting modules for environment '{}'", env);

    let mut chunk_graph = splitter.split(env);
    log::info!(
      "Created {} chunks for environment '{}'",
      chunk_graph.chunk_count(),
      env
    );

    // Emit the chunks
    let emitter = ChunkEmitter::new(&graph, emitter_config.clone());
    let emitted = emitter.emit_all(&mut chunk_graph)?;

    // Write to disk
    emitter.write_to_disk(&emitted)?;

    log::info!(
      "Emitted {} files to {}",
      emitted.len(),
      config.out_dir.display()
    );

    // Report what was generated
    for chunk in emitted {
      log::info!("  {} ({} bytes)", chunk.file_name, chunk.code.len());
    }
  }

  // Shutdown plugin host
  if let Some(host) = maybe_plugin_host {
    host.shutdown().await?;
  }

  Ok(())
}

/// Parse environment strings into BundleEnvironment values.
fn parse_environments(envs: &[String]) -> Vec<BundleEnvironment> {
  if envs.is_empty() {
    return vec![BundleEnvironment::Server];
  }
  envs
    .iter()
    .map(|s| BundleEnvironment::from_str(s))
    .collect()
}

/// Build the source module graph from entry points using the VFS.
async fn build_source_graph(
  config: &BuildConfig,
  vfs: &Arc<BundlerVirtualFS>,
) -> Result<SharedSourceGraph, AnyError> {
  let graph = SharedSourceGraph::new();

  // Add entry points
  for entry in &config.entry_points {
    for env in &config.environments {
      graph.write().add_entrypoint(env.clone(), entry.clone());
    }
  }

  // Process entry points and their dependencies
  let mut to_process: Vec<(ModuleSpecifier, Option<ModuleSpecifier>)> = config
    .entry_points
    .iter()
    .map(|e| (e.clone(), None))
    .collect();

  let mut processed = std::collections::HashSet::new();

  while let Some((specifier, referrer)) = to_process.pop() {
    if processed.contains(&specifier) {
      continue;
    }
    processed.insert(specifier.clone());

    // Load the module through VFS
    match load_module_via_vfs(&specifier, vfs).await {
      Ok(module) => {
        // Queue dependencies for processing
        for import in &module.imports {
          if !processed.contains(&import.specifier) {
            to_process
              .push((import.specifier.clone(), Some(specifier.clone())));
          }
        }
        for import in &module.dynamic_imports {
          if !processed.contains(&import.specifier) {
            to_process
              .push((import.specifier.clone(), Some(specifier.clone())));
          }
        }

        // Add environments to the module
        let mut module = module;
        for env in &config.environments {
          module.add_environment(env.clone());
        }

        // Mark entry points
        if config.entry_points.contains(&specifier) {
          module.is_entry = true;
        }

        graph.write().add_module(module);
      }
      Err(e) => {
        graph.write().add_error(source_graph::ModuleError {
          specifier: specifier.clone(),
          referrer,
          message: e.to_string(),
        });
      }
    }
  }

  Ok(graph)
}

/// Load a single module through the VFS.
async fn load_module_via_vfs(
  specifier: &ModuleSpecifier,
  vfs: &Arc<BundlerVirtualFS>,
) -> Result<SourceModule, AnyError> {
  // Use VFS to load (potentially transforming) the module
  let transformed = vfs.load(specifier).await?;

  // Create source module from transformed result
  let mut module = SourceModule::new(
    specifier.clone(),
    transformed.code.clone(),
    transformed.media_type,
  );

  // Store the transformed module if it was actually transformed
  if vfs.needs_transform(specifier) {
    module.transformed = Some(transformed.clone());
  }

  // Parse imports from the (potentially transformed) code
  // We use the media type after transformation (always JS/TS for transformed files)
  let analysis_media_type = if vfs.needs_transform(specifier) {
    // After transformation, the code is JavaScript
    deno_ast::MediaType::JavaScript
  } else {
    transformed.media_type
  };

  match import_analyzer::analyze_imports(
    specifier,
    &transformed.code,
    analysis_media_type,
  ) {
    Ok(analysis) => {
      module.imports = analysis.imports;
      module.dynamic_imports = analysis.dynamic_imports;
      module.re_exports = analysis.re_exports;
    }
    Err(e) => {
      // Log parse errors but don't fail the entire build
      log::warn!("Failed to analyze imports for {}: {}", specifier, e);
    }
  }

  Ok(module)
}
