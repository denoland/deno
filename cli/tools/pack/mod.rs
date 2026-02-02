// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use deno_ast::swc::ast as swc_ast;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
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

  // Check if git repository is clean (unless --allow-dirty)
  if !pack_flags.allow_dirty {
    check_git_status(cli_options.initial_cwd())?;
  }

  // Get package configs
  let mut packages = cli_options.start_dir.jsr_packages_for_publish();
  if packages.is_empty() {
    match cli_options.start_dir.member_deno_json() {
      Some(deno_json) => {
        if deno_json.json.name.is_none() {
          bail!(
            "Missing 'name' field in '{}'. Add a package name like:\n  {{\n    \"name\": \"@scope/package-name\",\n    ...\n  }}",
            deno_json.specifier
          );
        }
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
        let name = deno_json.json.name.clone().unwrap();

        // Validate package name format
        if !name.starts_with('@') || !name.contains('/') {
          bail!(
            "Invalid package name '{}' in '{}'. Package name must be in the format '@scope/name'",
            name,
            deno_json.specifier
          );
        }

        packages.push(JsrPackageConfig {
          name,
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
    let graph = create_graph(
      &module_graph_creator,
      &package,
      &pack_flags,
    )
    .await
    .with_context(|| {
      format!("Failed to build module graph for package '{}'", package.name)
    })?;

    // Collect files from the graph
    let collected_paths = collect_graph_modules(&graph, &package)?;

    log::info!("  {} modules collected", collected_paths.len());

    // Collect README and LICENSE files
    let readme_license_files = collect_readme_license_files(&package)?;

    // Process modules: transpile TS→JS, extract .d.ts
    let processed_files = process_modules(
      &graph,
      &collected_paths,
      parsed_source_cache.as_ref(),
      &pack_flags,
    )
    .with_context(|| {
      format!("Failed to process modules for package '{}'", package.name)
    })?;

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

async fn create_graph(
  module_graph_creator: &Arc<crate::graph_util::ModuleGraphCreator>,
  package: &JsrPackageConfig,
  pack_flags: &PackFlags,
) -> Result<ModuleGraph, AnyError> {
  use crate::args::config_to_deno_graph_workspace_member;
  use crate::graph_util::BuildFastCheckGraphOptions;
  use deno_graph::WorkspaceFastCheckOption;

  // Build initial graph without fast check DTS
  let mut graph = module_graph_creator
    .create_publish_graph(CreatePublishGraphOptions {
      packages: &[package.clone()],
      build_fast_check_graph: !pack_flags.allow_slow_types,
      validate_graph: true,
    })
    .await?;

  // If fast check is enabled, rebuild with DTS generation
  if !pack_flags.allow_slow_types {
    let fast_check_workspace_member =
      config_to_deno_graph_workspace_member(&package.config_file)?;

    module_graph_creator.module_graph_builder().build_fast_check_graph(
      &mut graph,
      BuildFastCheckGraphOptions {
        workspace_fast_check: WorkspaceFastCheckOption::Enabled(&[fast_check_workspace_member]),
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

pub struct ReadmeOrLicense {
  pub relative_path: String,
  pub content: Vec<u8>,
}

fn collect_readme_license_files(
  package: &JsrPackageConfig,
) -> Result<Vec<ReadmeOrLicense>, AnyError> {
  let package_dir = package.config_file.dir_path();
  let mut files = Vec::new();

  // Look for README files (case-insensitive)
  for name in &["README.md", "README", "readme.md", "Readme.md", "readme"] {
    let path = package_dir.join(name);
    if path.exists() {
      let content = std::fs::read(&path)?;
      files.push(ReadmeOrLicense {
        relative_path: name.to_string(),
        content,
      });
      break; // Only include one README
    }
  }

  // Look for LICENSE files (case-insensitive)
  for name in &["LICENSE", "LICENSE.md", "LICENCE", "LICENCE.md", "license", "license.md"] {
    let path = package_dir.join(name);
    if path.exists() {
      let content = std::fs::read(&path)?;
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
  #[allow(dead_code)]
  pub specifier: ModuleSpecifier,
  /// Relative path in the package (e.g., "mod.ts" -> "mod.js")
  pub output_path: String,
  /// Transpiled JS content (or original if not TS)
  pub js_content: String,
  /// Generated .d.ts content (if available)
  pub dts_content: Option<String>,
  /// Whether this file uses Deno APIs
  pub uses_deno: bool,
  /// Specific Deno APIs used in this file
  pub deno_apis_used: HashSet<String>,
  /// Extracted dependencies (package name -> version)
  pub dependencies: HashMap<String, String>,
}

/// Result of Deno API detection
#[derive(Debug, Default)]
struct DenoUsageInfo {
  /// Whether any Deno API usage was detected
  uses_deno: bool,
  /// Specific APIs used (e.g., "readFile", "env", "serve")
  apis_used: HashSet<String>,
  /// Whether import.meta.main is used
  uses_import_meta_main: bool,
}

/// Visitor to detect Deno API usage in AST
struct DenoUsageVisitor {
  info: DenoUsageInfo,
  /// Track locally-declared identifiers named "Deno" to avoid false positives
  local_deno_bindings: HashSet<String>,
}

impl DenoUsageVisitor {
  fn new() -> Self {
    Self {
      info: DenoUsageInfo::default(),
      local_deno_bindings: HashSet::new(),
    }
  }

  fn is_local_deno(&self, ident: &swc_ast::Ident) -> bool {
    self.local_deno_bindings.contains(&ident.to_id().0.to_string())
  }
}

impl Visit for DenoUsageVisitor {
  // Detect Deno.* member expressions
  fn visit_member_expr(&mut self, node: &swc_ast::MemberExpr) {
    // Check if this is accessing a property of Deno
    if let swc_ast::Expr::Ident(ident) = node.obj.as_ref() {
      if ident.sym.as_ref() == "Deno" && !self.is_local_deno(ident) {
        self.info.uses_deno = true;

        // Try to extract the specific API being accessed
        match &node.prop {
          swc_ast::MemberProp::Ident(prop_ident) => {
            self.info.apis_used.insert(prop_ident.sym.to_string());
          }
          swc_ast::MemberProp::Computed(computed) => {
            // Handle Deno["readFile"] style access
            if let swc_ast::Expr::Lit(swc_ast::Lit::Str(str_lit)) = computed.expr.as_ref() {
              self.info.apis_used.insert(str_lit.value.to_string_lossy().to_string());
            }
          }
          _ => {}
        }
      }
    }

    // Check for import.meta.main
    if let swc_ast::Expr::MetaProp(meta) = node.obj.as_ref() {
      if meta.kind == swc_ast::MetaPropKind::ImportMeta {
        if let swc_ast::MemberProp::Ident(prop) = &node.prop {
          if prop.sym.as_ref() == "main" {
            self.info.uses_import_meta_main = true;
          }
        }
      }
    }

    node.visit_children_with(self);
  }

  // Detect standalone Deno references (e.g., typeof Deno, const d = Deno)
  fn visit_ident(&mut self, node: &swc_ast::Ident) {
    if node.sym.as_ref() == "Deno" && !self.is_local_deno(node) {
      self.info.uses_deno = true;
    }
  }

  // Track local Deno declarations to avoid false positives
  fn visit_var_declarator(&mut self, node: &swc_ast::VarDeclarator) {
    if let swc_ast::Pat::Ident(ident) = &node.name {
      if ident.id.sym.as_ref() == "Deno" {
        self.local_deno_bindings.insert(ident.id.to_id().0.to_string());
      }
    }
    node.visit_children_with(self);
  }

  // Track function parameters named Deno
  fn visit_param(&mut self, node: &swc_ast::Param) {
    if let swc_ast::Pat::Ident(ident) = &node.pat {
      if ident.id.sym.as_ref() == "Deno" {
        self.local_deno_bindings.insert(ident.id.to_id().0.to_string());
      }
    }
    node.visit_children_with(self);
  }
}

/// Detect Deno API usage in a parsed source file using AST traversal
fn detect_deno_usage(parsed: &deno_ast::ParsedSource) -> DenoUsageInfo {
  let mut visitor = DenoUsageVisitor::new();
  let program = parsed.program_ref();
  program.visit_with(&mut visitor);
  visitor.info
}

/// APIs that are known to be unsupported or have limited support in @deno/shim-deno
const UNSUPPORTED_DENO_APIS: &[(&str, &str)] = &[
  ("dlopen", "FFI is not supported on Node.js"),
  ("bench", "benchmarking is Deno-specific; use a cross-runtime framework instead"),
  ("test", "testing is Deno-specific; use a cross-runtime testing framework instead"),
];

/// APIs that have partial support in @deno/shim-deno
const PARTIAL_SUPPORT_DENO_APIS: &[(&str, &str)] = &[
  ("serve", "has limited support; some features may not work"),
  ("listen", "has limited support; some features may not work"),
  ("listenTls", "has limited support; some features may not work"),
];

/// Emit warnings for unsupported or partially supported APIs
fn warn_about_deno_apis(
  file_path: &str,
  apis_used: &HashSet<String>,
  uses_import_meta_main: bool,
) {
  for (api, reason) in UNSUPPORTED_DENO_APIS {
    if apis_used.contains(*api) {
      log::warn!(
        "Deno.{} is used in {} but {}",
        api,
        file_path,
        reason
      );
    }
  }

  for (api, reason) in PARTIAL_SUPPORT_DENO_APIS {
    if apis_used.contains(*api) {
      log::warn!(
        "Deno.{} is used in {} and {}",
        api,
        file_path,
        reason
      );
    }
  }

  if uses_import_meta_main {
    log::warn!(
      "import.meta.main is used in {} but will always be undefined on Node.js",
      file_path
    );
  }
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

    // Detect Deno API usage using AST-based analysis (before transpiling consumes parsed)
    let deno_usage = detect_deno_usage(&parsed);

    // Warn about unsupported APIs
    if !pack_flags.no_shim {
      warn_about_deno_apis(
        &path.relative_path,
        &deno_usage.apis_used,
        deno_usage.uses_import_meta_main,
      );
    }

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

    // Inject Deno shim import if needed
    if deno_usage.uses_deno && !pack_flags.no_shim {
      js_content = format!("import {{ Deno }} from \"@deno/shim-deno\";\n{}", js_content);
    }

    // Extract .d.ts if available and not skipped
    let mut dts_content = if !pack_flags.allow_slow_types {
      extract_dts(js_module, media_type)
    } else {
      None
    };

    // Rewrite specifiers in .d.ts content too
    if let Some(ref dts) = dts_content {
      let (rewritten_dts, _) = rewrite_specifiers(
        dts,
        &path.specifier,
        graph,
      )?;
      dts_content = Some(rewritten_dts);
    }

    // Compute output path
    let output_path = compute_output_path(&path.relative_path, output_ext);

    processed.push(ProcessedFile {
      specifier: path.specifier.clone(),
      output_path,
      js_content,
      dts_content,
      uses_deno: deno_usage.uses_deno,
      deno_apis_used: deno_usage.apis_used,
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

      match deno_ast::emit(program_ref, &comments, &Default::default(), &emit_options) {
        Ok(emitted) => return Some(emitted.text),
        Err(e) => {
          log::warn!("Failed to emit DTS: {}", e);
          // Fall through to return fast check source
        }
      }
    }

    // Fallback: Return the fast check source (simplified TS)
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

fn check_git_status(cwd: &Path) -> Result<(), AnyError> {
  // Check if .git directory exists
  let git_dir = cwd.join(".git");
  if !git_dir.exists() {
    // Not a git repository, skip check
    return Ok(());
  }

  // Run git status --porcelain to check for uncommitted changes
  let output = std::process::Command::new("git")
    .arg("status")
    .arg("--porcelain")
    .current_dir(cwd)
    .output();

  match output {
    Ok(output) => {
      if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
          bail!(
            "Git repository has uncommitted changes. Use --allow-dirty to pack anyway.\n{}",
            stdout.trim()
          );
        }
      }
      // If git command fails, just warn but don't block
      Ok(())
    }
    Err(_) => {
      // Git not available or command failed, skip check
      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse_source(code: &str) -> deno_ast::ParsedSource {
    deno_ast::parse_module(deno_ast::ParseParams {
      specifier: deno_ast::ModuleSpecifier::parse("file:///test.ts").unwrap(),
      text: code.into(),
      media_type: deno_ast::MediaType::TypeScript,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .unwrap()
  }

  #[test]
  fn test_detect_deno_member_access() {
    let code = r#"
      const value = Deno.env.get("KEY");
      Deno.readTextFileSync("file.txt");
    "#;
    let parsed = parse_source(code);
    let info = detect_deno_usage(&parsed);
    assert!(info.uses_deno);
    assert!(info.apis_used.contains("env"));
    assert!(info.apis_used.contains("readTextFileSync"));
  }

  #[test]
  fn test_no_false_positive_comment() {
    let code = r#"
      // Visit Deno.land for more info
      const url = "https://deno.land";
    "#;
    let parsed = parse_source(code);
    let info = detect_deno_usage(&parsed);
    assert!(!info.uses_deno);
  }

  #[test]
  fn test_no_false_positive_string() {
    let code = r#"
      const message = "Deno.land is great";
      console.log("Check out Deno.env");
    "#;
    let parsed = parse_source(code);
    let info = detect_deno_usage(&parsed);
    assert!(!info.uses_deno);
  }

  #[test]
  fn test_detect_standalone_reference() {
    let code = r#"
      const runtime = Deno;
      if (typeof Deno !== "undefined") {
        console.log("Running on Deno");
      }
    "#;
    let parsed = parse_source(code);
    let info = detect_deno_usage(&parsed);
    assert!(info.uses_deno);
  }

  #[test]
  fn test_no_detect_local_binding() {
    let code = r#"
      const Deno = { custom: "object" };
      Deno.custom.toUpperCase();
    "#;
    let parsed = parse_source(code);
    let info = detect_deno_usage(&parsed);
    assert!(!info.uses_deno);
  }

  #[test]
  fn test_detect_computed_property() {
    let code = r#"
      const api = "readFile";
      Deno[api]("test.txt");
    "#;
    let parsed = parse_source(code);
    let info = detect_deno_usage(&parsed);
    assert!(info.uses_deno);
  }

  #[test]
  fn test_detect_import_meta_main() {
    let code = r#"
      if (import.meta.main) {
        console.log("Main module");
      }
    "#;
    let parsed = parse_source(code);
    let info = detect_deno_usage(&parsed);
    assert!(info.uses_import_meta_main);
  }

  #[test]
  fn test_nested_member_access() {
    let code = r#"
      const value = Deno.env.get("KEY");
      const cwd = Deno.cwd();
    "#;
    let parsed = parse_source(code);
    let info = detect_deno_usage(&parsed);
    assert!(info.uses_deno);
    assert!(info.apis_used.contains("env"));
    assert!(info.apis_used.contains("cwd"));
  }
}
