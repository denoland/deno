// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::diagnostics::Diagnostic;
use deno_config::workspace::JsrPackageConfig;
use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_terminal::colors;

use crate::args::FileFlagsExt;
use crate::args::Flags;
use crate::args::PackFlags;
use crate::factory::CliFactory;
use crate::graph_util::CreatePublishGraphOptions;
use crate::tools::lint::collect_no_slow_type_diagnostics;
use crate::util::display::human_size;

mod extensions;
mod npm_tarball;
mod package_json;
mod unfurl;

use extensions::compute_output_path;
use extensions::media_type_extension;
use npm_tarball::create_npm_tarball;
use package_json::generate_package_json;
use unfurl::unfurl_specifiers;

pub async fn pack(
  flags: Arc<Flags>,
  pack_flags: PackFlags,
) -> Result<(), AnyError> {
  let cli_factory = CliFactory::from_flags(flags);
  let cli_options = cli_factory.cli_options()?;

  // Check if git repository is clean (unless --allow-dirty)
  if !pack_flags.allow_dirty
    && let Some(dirty) =
      crate::util::git::check_if_git_repo_dirty(cli_options.initial_cwd()).await
  {
    bail!(
      "Git repository has uncommitted changes. Use --allow-dirty to pack anyway.\n{}",
      dirty
    );
  }

  // Get package configs
  let mut packages = cli_options.start_dir.jsr_packages_for_publish();
  if packages.is_empty() {
    match cli_options.start_dir.member_deno_json() {
      Some(deno_json) => {
        let Some(name) = deno_json.json.name.clone() else {
          bail!(
            "Missing 'name' field in '{}'. Add a package name like:\n  {{\n    \"name\": \"@scope/package-name\",\n    ...\n  }}",
            deno_json.specifier
          );
        };
        if deno_json.json.version.is_none() {
          bail!(
            "Missing 'version' field in '{}'. Add a version like:\n  {{\n    \"version\": \"1.0.0\",\n    ...\n  }}",
            deno_json.specifier
          );
        }
        if deno_json.json.exports.is_none() {
          bail!(
            "Missing 'exports' field in '{}'. Add an exports field like:\n  {{\n    \"exports\": \"./mod.ts\",\n    ...\n  }}",
            deno_json.specifier
          );
        }

        packages.push(JsrPackageConfig {
          name,
          member_dir: cli_options.start_dir.workspace.root_dir().clone(),
          config_file: deno_json.clone(),
          license: deno_json
            .json
            .license
            .as_ref()
            .and_then(|l| l.as_str().map(|s| s.to_string())),
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
    // Validate package name format
    if !package.name.starts_with('@') || !package.name.contains('/') {
      bail!(
        "Invalid package name '{}'. Package name must be in the format '@scope/name'",
        package.name
      );
    }

    log::info!(
      "{} {}",
      colors::green("Packing"),
      colors::intense_blue(&package.name)
    );

    // Determine version
    let version = if let Some(ref v) = pack_flags.set_version {
      if deno_semver::Version::parse_standard(v).is_err() {
        bail!(
          "Invalid semver version '{}'. Please provide a valid semver version (e.g., 1.0.0)",
          v
        );
      }
      v.clone()
    } else {
      package
        .config_file
        .json
        .version
        .clone()
        .with_context(|| {
          format!(
            "Missing version in package '{}'. Add a version field or use --set-version",
            package.name
          )
        })?
    };

    // Build module graph
    let graph = create_graph(module_graph_creator, &package, &pack_flags)
      .await
      .with_context(|| {
        format!(
          "Failed to build module graph for package '{}'",
          package.name
        )
      })?;
    warn_for_slow_type_diagnostics(&graph, &package, &pack_flags)?;

    // Collect files from the graph
    let collected_paths = collect_graph_modules(&graph, &package, &pack_flags)?;

    log::info!("  {} modules collected", collected_paths.len());

    // Collect README and LICENSE files
    let readme_license_files = collect_readme_license_files(&package)?;

    // Process modules: transpile TS→JS, extract .d.ts
    let processed_files = process_modules(
      &graph,
      &collected_paths,
      parsed_source_cache.as_ref(),
      &pack_flags,
      &package.config_file,
    )
    .with_context(|| {
      format!("Failed to process modules for package '{}'", package.name)
    })?;

    // Generate package.json
    let package_json =
      generate_package_json(&package.config_file, &version, &processed_files)?;

    // Create tarball
    let tarball_path = create_npm_tarball(
      &package.config_file,
      &version,
      &processed_files,
      &package_json,
      &readme_license_files,
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

fn warn_for_slow_type_diagnostics(
  graph: &ModuleGraph,
  package: &JsrPackageConfig,
  pack_flags: &PackFlags,
) -> Result<(), AnyError> {
  if pack_flags.allow_slow_types {
    return Ok(());
  }

  let export_urls = package.config_file.resolve_export_value_urls()?;
  for diagnostic in collect_no_slow_type_diagnostics(graph, &export_urls) {
    log::warn!("{}", diagnostic.display());
  }

  Ok(())
}

async fn create_graph(
  module_graph_creator: &Arc<crate::graph_util::ModuleGraphCreator>,
  package: &JsrPackageConfig,
  pack_flags: &PackFlags,
) -> Result<ModuleGraph, AnyError> {
  use deno_graph::WorkspaceFastCheckOption;

  use crate::args::config_to_deno_graph_workspace_member;
  use crate::graph_util::BuildFastCheckGraphOptions;

  // Build initial graph without fast check DTS
  let mut graph = module_graph_creator
    .create_publish_graph(CreatePublishGraphOptions {
      packages: std::slice::from_ref(package),
      build_fast_check_graph: !pack_flags.allow_slow_types,
      validate_graph: true,
    })
    .await?;

  // If fast check is enabled, rebuild with DTS generation
  if !pack_flags.allow_slow_types {
    let fast_check_workspace_member =
      config_to_deno_graph_workspace_member(&package.config_file)?;

    module_graph_creator
      .module_graph_builder()
      .build_fast_check_graph(
        &mut graph,
        BuildFastCheckGraphOptions {
          workspace_fast_check: WorkspaceFastCheckOption::Enabled(&[
            fast_check_workspace_member,
          ]),
          fast_check_dts: true,
        },
      )?;
  }

  Ok(graph)
}

struct CollectedPath {
  specifier: ModuleSpecifier,
  relative_path: String,
}

fn collect_graph_modules(
  graph: &ModuleGraph,
  package: &JsrPackageConfig,
  pack_flags: &PackFlags,
) -> Result<Vec<CollectedPath>, AnyError> {
  let package_dir = &package.config_file.dir_path();
  let mut paths = Vec::new();

  // Create file patterns from pack_flags
  let file_patterns = pack_flags.files.as_file_patterns(package_dir)?;

  for module in graph.modules() {
    if let Module::Js(js_module) = module {
      let specifier = &js_module.specifier;

      // Only include file: URLs in the package directory
      if specifier.scheme() == "file"
        && let Ok(path) = specifier.to_file_path()
        && path.starts_with(package_dir)
        && file_patterns.matches_path(&path, deno_config::glob::PathKind::File)
        && !is_excluded_path(&path, package_dir)
      {
        let relative = path.strip_prefix(package_dir).unwrap();
        paths.push(CollectedPath {
          specifier: specifier.clone(),
          relative_path: relative.to_string_lossy().to_string(),
        });
      }
    }
  }

  // `graph.modules()` walks deno_graph's internal map, whose iteration
  // order is not guaranteed across runs. Sort by relative path so the
  // tarball entries land in a stable order — without this the tar bytes
  // would drift between runs even when package.json itself is
  // deterministic. Covered by the `reproducible` spec test.
  paths.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

  Ok(paths)
}

/// Hard-exclude well-known directories that should never end up in an
/// npm tarball, regardless of the user's `files`/`exclude` patterns.
/// Mirrors `npm pack`'s default ignores so we don't accidentally
/// publish secrets or VCS state via local `file:` imports under the
/// package directory.
fn is_excluded_path(
  path: &std::path::Path,
  package_dir: &std::path::Path,
) -> bool {
  let Ok(rel) = path.strip_prefix(package_dir) else {
    return false;
  };
  for component in rel.components() {
    let std::path::Component::Normal(name) = component else {
      continue;
    };
    let Some(name) = name.to_str() else { continue };
    if name == ".git" || name == "node_modules" {
      return true;
    }
    // .env, .env.local, .env.production, etc.
    if name == ".env" || name.starts_with(".env.") {
      return true;
    }
  }
  false
}

pub struct ReadmeOrLicense {
  pub relative_path: String,
  pub content: Vec<u8>,
}

/// Read an auto-included file (README/LICENSE) only if it is a regular
/// file in the package directory. We use `symlink_metadata` rather than
/// `Path::exists()` + `read()` so a symlink pointing outside the
/// package — e.g. a `LICENSE` symlink to `~/.ssh/id_rsa` — never gets
/// packed. Returns `Ok(None)` if the path does not exist or is not a
/// regular file; returns `Err` only on actual I/O failure when reading
/// a confirmed regular file.
fn read_auto_included_file(
  path: &std::path::Path,
) -> Result<Option<Vec<u8>>, AnyError> {
  let metadata = match std::fs::symlink_metadata(path) {
    Ok(m) => m,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(e) => return Err(e.into()),
  };
  if !metadata.file_type().is_file() {
    bail!(
      "Refusing to include {}: not a regular file (symlinks and special files are excluded)",
      path.display()
    );
  }
  Ok(Some(std::fs::read(path)?))
}

fn collect_readme_license_files(
  package: &JsrPackageConfig,
) -> Result<Vec<ReadmeOrLicense>, AnyError> {
  let package_dir = package.config_file.dir_path();
  let mut files = Vec::new();

  // Look for README files (case-insensitive)
  for name in &["README.md", "README", "readme.md", "Readme.md", "readme"] {
    let path = package_dir.join(name);
    if let Some(content) = read_auto_included_file(&path)? {
      files.push(ReadmeOrLicense {
        relative_path: name.to_string(),
        content,
      });
      break; // Only include one README
    }
  }

  // Look for LICENSE files (common variants)
  for name in &[
    "LICENSE",
    "LICENSE.md",
    "LICENSE.txt",
    "LICENCE",
    "LICENCE.md",
    "LICENCE.txt",
    "license",
    "license.md",
    "license.txt",
  ] {
    let path = package_dir.join(name);
    if let Some(content) = read_auto_included_file(&path)? {
      files.push(ReadmeOrLicense {
        relative_path: name.to_string(),
        content,
      });
      break; // Only include one LICENSE
    }
  }

  Ok(files)
}

pub struct ProcessedFile {
  /// Original specifier
  #[allow(dead_code, reason = "kept for debugging and future use")]
  pub specifier: ModuleSpecifier,
  /// Relative path in the package (e.g., "mod.ts" -> "mod.js")
  pub output_path: String,
  /// Transpiled JS content (or original if not TS)
  pub js_content: String,
  /// Generated .d.ts content (if available)
  pub dts_content: Option<String>,
  /// Extracted dependencies (package name -> version). BTreeMap so the
  /// merged dependencies in package.json come out in sorted order across
  /// runs (reproducibility).
  pub dependencies: BTreeMap<String, String>,
}

/// Split a leading shebang line off the source so we can preserve it as
/// the first line of the emitted JS file without it interfering with
/// transpilation. Returns `(shebang_with_newline, rest_of_source)`.
fn split_shebang(source: &str) -> (Option<&str>, &str) {
  if !source.starts_with("#!") {
    return (None, source);
  }
  match source.find('\n') {
    Some(nl) => (Some(&source[..=nl]), &source[nl + 1..]),
    None => (Some(source), ""),
  }
}

/// Create transpile options from deno.json compiler options
fn create_transpile_options(
  config_file: &deno_config::deno_json::ConfigFile,
) -> Result<deno_ast::TranspileOptions, AnyError> {
  // Get compiler options from deno.json
  let compiler_options = config_file.json.compiler_options.as_ref();

  // Helper to extract a string value from compiler options
  let get_str = |key: &str| -> Option<String> {
    compiler_options?.get(key)?.as_str().map(|s| s.to_string())
  };

  // Extract JSX settings
  let jsx = get_str("jsx");
  let jsx_import_source = get_str("jsxImportSource");
  let jsx_factory = get_str("jsxFactory");
  let jsx_fragment_factory = get_str("jsxFragmentFactory");

  let jsx_runtime = match jsx.as_deref() {
    Some("react") => {
      Some(deno_ast::JsxRuntime::Classic(deno_ast::JsxClassicOptions {
        factory: jsx_factory
          .unwrap_or_else(|| "React.createElement".to_string()),
        fragment_factory: jsx_fragment_factory
          .unwrap_or_else(|| "React.Fragment".to_string()),
      }))
    }
    Some("react-jsx") => Some(deno_ast::JsxRuntime::Automatic(
      deno_ast::JsxAutomaticOptions {
        development: false,
        import_source: jsx_import_source,
      },
    )),
    Some("react-jsxdev") => Some(deno_ast::JsxRuntime::Automatic(
      deno_ast::JsxAutomaticOptions {
        development: true,
        import_source: jsx_import_source.clone(),
      },
    )),
    Some("precompile") => Some(deno_ast::JsxRuntime::Precompile(
      deno_ast::JsxPrecompileOptions {
        automatic: deno_ast::JsxAutomaticOptions {
          development: false,
          import_source: jsx_import_source,
        },
        skip_elements: None,
        dynamic_props: None,
      },
    )),
    _ => None,
  };

  // Extract decorator settings
  let get_bool = |key: &str| -> bool {
    compiler_options
      .and_then(|opts| opts.get(key))
      .and_then(|v| v.as_bool())
      .unwrap_or(false)
  };
  let experimental_decorators = get_bool("experimentalDecorators");
  let emit_decorator_metadata = get_bool("emitDecoratorMetadata");

  Ok(deno_ast::TranspileOptions {
    jsx: jsx_runtime,
    decorators: if experimental_decorators {
      deno_ast::DecoratorsTranspileOption::LegacyTypeScript {
        emit_metadata: emit_decorator_metadata,
      }
    } else {
      deno_ast::DecoratorsTranspileOption::Ecma
    },
    imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
    var_decl_imports: false,
    verbatim_module_syntax: false,
  })
}

fn process_modules(
  graph: &ModuleGraph,
  paths: &[CollectedPath],
  parsed_source_cache: &deno_resolver::cache::ParsedSourceCache,
  pack_flags: &PackFlags,
  config_file: &deno_config::deno_json::ConfigFile,
) -> Result<Vec<ProcessedFile>, AnyError> {
  let mut processed = Vec::new();

  // Get transpile options from deno.json compiler options
  let transpile_options = create_transpile_options(config_file)?;

  for path in paths {
    let module = graph.get(&path.specifier);
    let Some(Module::Js(js_module)) = module else {
      continue;
    };

    let file = process_single_module(
      graph,
      js_module,
      path,
      parsed_source_cache,
      pack_flags,
      &transpile_options,
    )?;
    processed.push(file);
  }

  Ok(processed)
}

fn process_single_module(
  graph: &ModuleGraph,
  js_module: &deno_graph::JsModule,
  path: &CollectedPath,
  parsed_source_cache: &deno_resolver::cache::ParsedSourceCache,
  pack_flags: &PackFlags,
  transpile_options: &deno_ast::TranspileOptions,
) -> Result<ProcessedFile, AnyError> {
  let media_type = js_module.media_type;
  let raw_source = js_module.source.text.as_ref();
  // Strip a leading shebang (e.g. `#!/usr/bin/env deno`) before all other
  // processing. SWC's module parser rejects shebangs with "Expected
  // ident", and tools downstream don't want to deal with them either.
  // We re-prepend the shebang to the final emitted JS so executable
  // scripts still work after `deno pack`.
  let (shebang, source_text) = split_shebang(raw_source);

  let parsed = parsed_source_cache.remove_or_parse_module(
    &path.specifier,
    media_type,
    source_text.into(),
  )?;

  // Phase 1: Collect AST-based text changes (specifier rewriting)
  // This happens BEFORE transpilation so source maps stay accurate.
  let (source_to_transpile, dependencies) =
    if media_type.is_emittable() || media_type == MediaType::JavaScript {
      let unfurl_result = unfurl_specifiers(&parsed, &path.specifier, graph);
      let text_changes = unfurl_result.text_changes;

      if text_changes.is_empty() {
        (source_text.to_string(), unfurl_result.dependencies)
      } else {
        let text_info = parsed.text_info_lazy();
        let modified =
          deno_ast::apply_text_changes(text_info.text_str(), text_changes);
        (modified, unfurl_result.dependencies)
      }
    } else {
      (source_text.to_string(), BTreeMap::new())
    };

  // Phase 2: Transpile the modified source (with rewritten specifiers)
  let (js_content, output_ext) = if media_type.is_emittable() {
    let source_map_option = if pack_flags.no_source_maps {
      deno_ast::SourceMapOption::None
    } else {
      deno_ast::SourceMapOption::Inline
    };

    let modified_parsed = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: path.specifier.clone(),
      text: source_to_transpile.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })?;

    let transpiled = modified_parsed.transpile(
      transpile_options,
      &deno_ast::TranspileModuleOptions::default(),
      &deno_ast::EmitOptions {
        source_map: source_map_option,
        inline_sources: true,
        ..Default::default()
      },
    )?;
    let text = transpiled.into_source().text;
    let ext = match media_type {
      MediaType::Mts => ".mjs",
      MediaType::Cts => ".cjs",
      _ => ".js",
    };
    (text, ext)
  } else {
    // Non-emittable files: use the (possibly rewritten) source directly
    (source_to_transpile, media_type_extension(media_type))
  };

  // Re-prepend the shebang we stripped earlier so executable scripts
  // (e.g. `#!/usr/bin/env -S deno run -A`) still work after pack.
  let js_content = match shebang {
    Some(line) => format!("{}{}", line, js_content),
    None => js_content,
  };

  // Extract .d.ts if available and not skipped, then unfurl its specifiers
  let dts_content = if !pack_flags.allow_slow_types {
    let dts = extract_dts(js_module, media_type);
    dts.map(|dts_text| unfurl_dts_content(dts_text, &path.specifier, graph))
  } else {
    None
  };

  // Compute output path
  let output_path = compute_output_path(&path.relative_path, output_ext);

  Ok(ProcessedFile {
    specifier: path.specifier.clone(),
    output_path,
    js_content,
    dts_content,
    dependencies,
  })
}

/// Parse and unfurl specifiers in generated .d.ts content.
fn unfurl_dts_content(
  dts_text: String,
  specifier: &ModuleSpecifier,
  graph: &ModuleGraph,
) -> String {
  let dts_parsed = deno_ast::parse_module(deno_ast::ParseParams {
    specifier: specifier.clone(),
    text: dts_text.clone().into(),
    media_type: MediaType::Dts,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  });
  match dts_parsed {
    Ok(dts_parsed) => {
      let dts_unfurl = unfurl_specifiers(&dts_parsed, specifier, graph);
      if dts_unfurl.text_changes.is_empty() {
        dts_text
      } else {
        let text_info = dts_parsed.text_info_lazy();
        deno_ast::apply_text_changes(
          text_info.text_str(),
          dts_unfurl.text_changes,
        )
      }
    }
    Err(e) => {
      log::warn!("Failed to parse .d.ts for specifier rewriting: {}", e);
      dts_text
    }
  }
}

fn extract_dts(
  js_module: &deno_graph::JsModule,
  media_type: MediaType,
) -> Option<String> {
  // Only generate .d.ts for typed files
  if !media_type.is_typed() {
    return None;
  }

  // Try to get fast check module with DTS
  if let Some(fast_check) = js_module.fast_check_module() {
    // Check if we have a separate DTS module
    if let Some(ref dts_module) = fast_check.dts {
      // Emit the DTS program to a string
      let emit_options = deno_ast::EmitOptions {
        source_map: deno_ast::SourceMapOption::None,
        ..Default::default()
      };

      // Convert program to ProgramRef and comments to single-threaded
      let program_ref = (&dts_module.program).into();
      let comments = dts_module.comments.as_single_threaded();

      match deno_ast::emit(
        program_ref,
        &comments,
        &Default::default(),
        &emit_options,
      ) {
        Ok(emitted) => return Some(emitted.text),
        Err(e) => {
          // No fall-through to fast_check.source: it's simplified
          // TypeScript, not valid .d.ts. We return None so the .d.ts
          // file is omitted from the tarball entirely and the
          // generated package.json drops its `types` field for this
          // entry, rather than pointing at an empty `export {};` stub
          // that would make TypeScript conclude the module exports
          // nothing.
          log::warn!(
            "Failed to emit .d.ts for '{}': {}. Types will not be included for this module.",
            js_module.specifier,
            e
          );
          return None;
        }
      }
    }

    log::warn!(
      "Could not generate .d.ts for '{}': fast check produced no DTS module. Types will not be included for this module.",
      js_module.specifier
    );
    return None;
  }

  log::warn!(
    "Could not generate types for '{}'. Types will not be included for this module.",
    js_module.specifier
  );
  None
}
