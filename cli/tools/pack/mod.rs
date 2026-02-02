// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::workspace::JsrPackageConfig;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_terminal::colors;

use crate::args::Flags;
use crate::args::PackFlags;
use crate::factory::CliFactory;
use crate::graph_util::CreatePublishGraphOptions;
use crate::util::display::human_size;

mod npm_tarball;
mod package_json;
mod specifier_rewriter;

use npm_tarball::create_npm_tarball;
use package_json::generate_package_json;
use specifier_rewriter::rewrite_specifiers;

pub async fn pack(
  flags: Arc<Flags>,
  pack_flags: PackFlags,
) -> Result<(), AnyError> {
  let cli_factory = CliFactory::from_flags(flags);
  let cli_options = cli_factory.cli_options()?;

  // Get package configs
  let mut packages = cli_options.start_dir.jsr_packages_for_publish();
  if packages.is_empty() {
    match cli_options.start_dir.member_deno_json() {
      Some(deno_json) => {
        if deno_json.json.name.is_none() {
          bail!("Missing 'name' field in deno.json");
        }
        if deno_json.json.version.is_none() {
          bail!("Missing 'version' field in deno.json");
        }
        if deno_json.json.exports.is_none() {
          bail!("Missing 'exports' field in deno.json");
        }
        packages.push(JsrPackageConfig {
          name: deno_json.json.name.clone().unwrap(),
          member_dir: cli_options.start_dir.workspace.root_dir().clone(),
          config_file: deno_json.clone(),
          license: deno_json.json.license.as_ref().and_then(|l| {
            l.as_str().map(|s| s.to_string())
          }),
          should_publish: true,
        });
      }
      None => {
        bail!("No deno.json found in current directory");
      }
    }
  }

  let module_graph_creator = cli_factory.module_graph_creator().await?;
  let parsed_source_cache = cli_factory.parsed_source_cache()?;

  for package in packages {
    log::info!(
      "{} {}",
      colors::green("Packing"),
      colors::intense_blue(&package.name)
    );

    // Determine version
    let version = if let Some(ref v) = pack_flags.set_version {
      v.clone()
    } else {
      package
        .config_file
        .json
        .version
        .clone()
        .context("Missing version")?
    };

    // Build module graph
    let graph = create_graph(
      &module_graph_creator,
      &package,
      &pack_flags,
    )
    .await?;

    // Collect files from the graph
    let collected_paths = collect_graph_modules(&graph, &package)?;

    log::info!("  {} modules collected", collected_paths.len());

    // Process modules: transpile TS→JS, extract .d.ts
    let processed_files = process_modules(
      &graph,
      &collected_paths,
      parsed_source_cache.as_ref(),
      &pack_flags,
    )?;

    // Detect Deno API usage
    let uses_deno_api = detect_deno_api_usage(&processed_files);

    // Generate package.json
    let package_json = generate_package_json(
      &package.config_file,
      &version,
      &processed_files,
      uses_deno_api && !pack_flags.no_shim,
    )?;

    // Create tarball
    let tarball_path = create_npm_tarball(
      &package.config_file,
      &version,
      &processed_files,
      &package_json,
      pack_flags.output.as_deref(),
      pack_flags.dry_run,
    )?;

    if pack_flags.dry_run {
      log::info!("{} Dry run - no tarball created", colors::green("✓"));
    } else {
      let metadata = std::fs::metadata(&tarball_path)?;
      log::info!(
        "{} {} ({})",
        colors::green("✓"),
        tarball_path.display(),
        human_size(metadata.len() as f64)
      );
    }
  }

  Ok(())
}

async fn create_graph(
  module_graph_creator: &Arc<crate::graph_util::ModuleGraphCreator>,
  package: &JsrPackageConfig,
  pack_flags: &PackFlags,
) -> Result<ModuleGraph, AnyError> {
  // Build graph with fast check enabled (which generates dts)
  let graph = module_graph_creator
    .create_publish_graph(CreatePublishGraphOptions {
      packages: &[package.clone()],
      build_fast_check_graph: !pack_flags.allow_slow_types,
      validate_graph: true,
    })
    .await?;

  Ok(graph)
}

struct CollectedPath {
  specifier: ModuleSpecifier,
  relative_path: String,
}

fn collect_graph_modules(
  graph: &ModuleGraph,
  package: &JsrPackageConfig,
) -> Result<Vec<CollectedPath>, AnyError> {
  let package_dir = &package.config_file.dir_path();
  let mut paths = Vec::new();

  for module in graph.modules() {
    if let Module::Js(js_module) = module {
      let specifier = &js_module.specifier;

      // Only include file: URLs in the package directory
      if specifier.scheme() == "file" {
        if let Ok(path) = specifier.to_file_path() {
          if path.starts_with(package_dir) {
            let relative = path.strip_prefix(package_dir).unwrap();
            paths.push(CollectedPath {
              specifier: specifier.clone(),
              relative_path: relative.to_string_lossy().to_string(),
            });
          }
        }
      }
    }
  }

  Ok(paths)
}

pub struct ProcessedFile {
  /// Original specifier
  pub specifier: ModuleSpecifier,
  /// Relative path in the package (e.g., "mod.ts" -> "mod.js")
  pub output_path: String,
  /// Transpiled JS content (or original if not TS)
  pub js_content: String,
  /// Generated .d.ts content (if available)
  pub dts_content: Option<String>,
  /// Whether this file uses Deno APIs
  pub uses_deno: bool,
  /// Extracted dependencies (package name -> version)
  pub dependencies: HashMap<String, String>,
}

fn process_modules(
  graph: &ModuleGraph,
  paths: &[CollectedPath],
  parsed_source_cache: &deno_resolver::cache::ParsedSourceCache,
  pack_flags: &PackFlags,
) -> Result<Vec<ProcessedFile>, AnyError> {
  let mut processed = Vec::new();

  for path in paths {
    let module = graph.get(&path.specifier);
    let Some(Module::Js(js_module)) = module else {
      continue;
    };

    let media_type = js_module.media_type;
    let source_text = js_module.source.text.as_ref();

    // Parse and transpile
    let parsed = parsed_source_cache.remove_or_parse_module(
      &path.specifier,
      media_type,
      source_text.into(),
    )?;

    // Transpile if needed
    let (mut js_content, output_ext) = if media_type.is_emittable() {
      let transpiled = parsed.transpile(
        &deno_ast::TranspileOptions::default(),
        &deno_ast::TranspileModuleOptions::default(),
        &deno_ast::EmitOptions {
          source_map: deno_ast::SourceMapOption::None,
          ..Default::default()
        },
      )?;
      let text = transpiled.into_source().text;
      let ext = if media_type == MediaType::Mts { ".mjs" } else { ".js" };
      (text, ext)
    } else {
      // Pass through non-emittable files
      (source_text.to_string(), get_extension(media_type))
    };

    // Rewrite specifiers in the JS content
    let dependencies = if media_type.is_emittable() || media_type == MediaType::JavaScript {
      let (rewritten_content, deps) = rewrite_specifiers(
        &js_content,
        &path.specifier,
        graph,
      )?;

      js_content = rewritten_content;
      deps
    } else {
      HashMap::new()
    };

    // Extract .d.ts if available and not skipped
    let dts_content = if !pack_flags.allow_slow_types {
      extract_dts(js_module, media_type)
    } else {
      None
    };

    // Detect Deno API usage
    let uses_deno = source_text.contains("Deno.");

    // Compute output path
    let output_path = compute_output_path(&path.relative_path, output_ext);

    processed.push(ProcessedFile {
      specifier: path.specifier.clone(),
      output_path,
      js_content,
      dts_content,
      uses_deno,
      dependencies,
    });
  }

  Ok(processed)
}

fn extract_dts(
  js_module: &deno_graph::JsModule,
  media_type: MediaType,
) -> Option<String> {
  // Only generate .d.ts for typed files
  if !media_type.is_typed() {
    return None;
  }

  // Try to get fast check module
  if let Some(fast_check) = js_module.fast_check_module() {
    // Return the fast check source directly
    return Some(fast_check.source.as_ref().to_string());
  }

  // Fallback: generate a stub
  Some("export {};".to_string())
}

fn compute_output_path(relative_path: &str, new_ext: &str) -> String {
  let path = Path::new(relative_path);
  let stem = path.file_stem().unwrap().to_str().unwrap();
  let parent = path.parent().unwrap_or(Path::new(""));

  if parent == Path::new("") {
    format!("{}{}", stem, new_ext)
  } else {
    format!("{}/{}{}", parent.display(), stem, new_ext)
  }
}

fn get_extension(media_type: MediaType) -> &'static str {
  match media_type {
    MediaType::JavaScript => ".js",
    MediaType::Jsx => ".jsx",
    MediaType::Mjs => ".mjs",
    MediaType::Cjs => ".cjs",
    MediaType::Json => ".json",
    _ => ".js",
  }
}

fn detect_deno_api_usage(files: &[ProcessedFile]) -> bool {
  files.iter().any(|f| f.uses_deno)
}
