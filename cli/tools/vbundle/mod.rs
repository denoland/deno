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

pub mod environment;
pub mod plugins;
pub mod source_graph;
pub mod types;

pub use environment::BundleEnvironment;
pub use plugins::create_runner_and_load_plugins;
pub use plugins::PluginHostProxy;
pub use plugins::PluginLogger;
pub use source_graph::SharedSourceGraph;
pub use source_graph::SourceModule;
pub use types::BuildConfig;
pub use types::TransformedModule;

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
    .map(|f| {
      deno_path_util::resolve_url_or_path(f, cli_options.initial_cwd())
    })
    .collect::<Result<Vec<_>, _>>()?;

  if entry_points.is_empty() {
    log::error!("No entry points specified");
    return Err(deno_core::anyhow::anyhow!("No entry points specified"));
  }

  // Parse plugin specifiers
  let plugin_specifiers: Vec<ModuleSpecifier> = vbundle_flags
    .plugins
    .iter()
    .map(|p| {
      deno_path_util::resolve_url_or_path(p, cli_options.initial_cwd())
    })
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

  log::info!("Starting vbundle with {} entry points", config.entry_points.len());

  // Create plugin host if there are plugins
  let maybe_plugin_host = if !plugin_specifiers.is_empty() {
    let logger = PluginLogger::new(|msg, is_error| {
      if is_error {
        eprintln!("{}", msg);
      } else {
        println!("{}", msg);
      }
    });
    let host = create_runner_and_load_plugins(plugin_specifiers, logger).await?;
    log::info!("Loaded {} plugins", host.get_plugins().len());
    Some(Arc::new(host))
  } else {
    None
  };

  // Build the source module graph
  let graph = build_source_graph(&config, maybe_plugin_host.as_ref()).await?;

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

  // TODO: Phase 4 - Code splitting and chunk generation
  // TODO: Phase 5 - Multi-environment chunk graphs
  // TODO: Code emission with deno_ast

  // For now, just report what we found
  for env in graph.read().environments() {
    let module_count = graph.read().modules_for_env(env).count();
    log::info!("Environment '{}': {} modules", env, module_count);
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
  envs.iter().map(|s| BundleEnvironment::from_str(s)).collect()
}

/// Build the source module graph from entry points.
async fn build_source_graph(
  config: &BuildConfig,
  maybe_plugin_host: Option<&Arc<PluginHostProxy>>,
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

    // Try to load the module
    match load_module(&specifier, referrer.as_ref(), maybe_plugin_host).await {
      Ok(module) => {
        // Queue dependencies for processing
        for import in &module.imports {
          if !processed.contains(&import.specifier) {
            to_process.push((import.specifier.clone(), Some(specifier.clone())));
          }
        }
        for import in &module.dynamic_imports {
          if !processed.contains(&import.specifier) {
            to_process.push((import.specifier.clone(), Some(specifier.clone())));
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

/// Load a single module, using plugins if available.
async fn load_module(
  specifier: &ModuleSpecifier,
  _referrer: Option<&ModuleSpecifier>,
  maybe_plugin_host: Option<&Arc<PluginHostProxy>>,
) -> Result<SourceModule, AnyError> {
  let id = specifier.as_str();

  // Try plugin load hook first
  if let Some(host) = maybe_plugin_host {
    if let Some(load_result) = host.load(id).await? {
      let media_type = match load_result.loader.as_deref() {
        Some("ts") | Some("typescript") => deno_ast::MediaType::TypeScript,
        Some("tsx") => deno_ast::MediaType::Tsx,
        Some("jsx") => deno_ast::MediaType::Jsx,
        Some("json") => deno_ast::MediaType::Json,
        _ => deno_ast::MediaType::JavaScript,
      };

      let source: Arc<str> = load_result.code.into();
      let mut module = SourceModule::new(specifier.clone(), source, media_type);

      // TODO: Parse imports from the loaded code
      // For now, we return the module without analyzing imports

      return Ok(module);
    }
  }

  // Fall back to native loading
  let source = load_native(specifier).await?;
  let media_type = deno_ast::MediaType::from_specifier(specifier);
  let mut module = SourceModule::new(specifier.clone(), source, media_type);

  // Try plugin transform hook
  if let Some(host) = maybe_plugin_host {
    if let Some(transform_result) = host.transform(id, &module.source).await? {
      let transformed = TransformedModule {
        original_specifier: specifier.clone(),
        code: transform_result.code.into(),
        source_map: None, // TODO: Parse source map
        media_type: deno_ast::MediaType::JavaScript,
        declarations: None,
      };
      module.transformed = Some(transformed);
    }
  }

  // TODO: Parse imports from the source code using deno_ast

  Ok(module)
}

/// Load a module from the file system or network.
async fn load_native(specifier: &ModuleSpecifier) -> Result<Arc<str>, AnyError> {
  if specifier.scheme() == "file" {
    let path = specifier
      .to_file_path()
      .map_err(|_| deno_core::anyhow::anyhow!("Invalid file URL: {}", specifier))?;
    let content = tokio::fs::read_to_string(&path).await?;
    Ok(content.into())
  } else {
    // For remote modules, we would use the module loader
    // For now, return an error
    Err(deno_core::anyhow::anyhow!(
      "Remote modules not yet supported: {}",
      specifier
    ))
  }
}
