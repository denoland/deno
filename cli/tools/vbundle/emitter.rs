// Copyright 2018-2026 the Deno authors. MIT license.

//! Code emission for bundled chunks.
//!
//! This module handles generating the final bundled JavaScript code from
//! chunks. It uses deno_ast for code transformation and generation.
//!
//! # Emission Strategy
//!
//! For each chunk:
//! 1. Wrap each module in a function scope
//! 2. Rewrite imports to reference the bundled modules
//! 3. Concatenate all module functions
//! 4. Generate source maps

use std::collections::HashMap;

use deno_ast::EmitOptions;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseParams;
use deno_ast::SourceMapOption;
use deno_ast::TranspileModuleOptions;
use deno_ast::TranspileOptions;
use deno_core::error::AnyError;

use super::chunk_graph::Chunk;
use super::chunk_graph::ChunkGraph;
use super::chunk_graph::ChunkId;
use super::environment::BundleEnvironment;
use super::hmr_runtime;
use super::hmr_types::HmrConfig;
use super::source_graph::SharedSourceGraph;
use super::splitter::determine_bundle_order;

/// Configuration for the emitter.
#[derive(Debug, Clone)]
pub struct EmitterConfig {
  /// Whether to generate source maps.
  pub source_maps: bool,

  /// Whether to minify the output.
  pub minify: bool,

  /// The output directory.
  pub out_dir: std::path::PathBuf,

  /// Environment variables to inject (for import.meta.env).
  pub env_vars: HashMap<String, String>,

  /// The build mode (development or production).
  pub mode: BuildMode,

  /// Whether to enable HMR (Hot Module Replacement).
  pub hmr: bool,

  /// HMR configuration (only used if hmr is true).
  pub hmr_config: Option<HmrConfig>,
}

/// Build mode for environment-specific transforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildMode {
  #[default]
  Development,
  Production,
}

impl Default for EmitterConfig {
  fn default() -> Self {
    Self {
      source_maps: true,
      minify: false,
      out_dir: std::path::PathBuf::from("dist"),
      env_vars: HashMap::new(),
      mode: BuildMode::Development,
      hmr: false,
      hmr_config: None,
    }
  }
}

impl EmitterConfig {
  /// Create a production config.
  pub fn production() -> Self {
    Self {
      mode: BuildMode::Production,
      minify: true,
      hmr: false,
      hmr_config: None,
      ..Default::default()
    }
  }

  /// Enable HMR with default configuration.
  pub fn with_hmr(mut self) -> Self {
    self.hmr = true;
    self.hmr_config = Some(HmrConfig::default());
    self
  }

  /// Enable HMR with custom configuration.
  pub fn with_hmr_config(mut self, config: HmrConfig) -> Self {
    self.hmr = true;
    self.hmr_config = Some(config);
    self
  }

  /// Set environment variables for import.meta.env.
  pub fn with_env_vars(mut self, vars: HashMap<String, String>) -> Self {
    self.env_vars = vars;
    self
  }

  /// Add a single environment variable.
  pub fn with_env_var(
    mut self,
    key: impl Into<String>,
    value: impl Into<String>,
  ) -> Self {
    self.env_vars.insert(key.into(), value.into());
    self
  }
}

/// Result of emitting a chunk.
#[derive(Debug)]
pub struct EmittedChunk {
  /// The chunk ID.
  pub chunk_id: ChunkId,

  /// The file name.
  pub file_name: String,

  /// The generated code.
  pub code: String,

  /// The source map (if generated).
  pub source_map: Option<String>,
}

/// Code emitter for bundled chunks.
pub struct ChunkEmitter<'a> {
  source_graph: &'a SharedSourceGraph,
  config: EmitterConfig,
}

impl<'a> ChunkEmitter<'a> {
  /// Create a new chunk emitter.
  pub fn new(
    source_graph: &'a SharedSourceGraph,
    config: EmitterConfig,
  ) -> Self {
    Self {
      source_graph,
      config,
    }
  }

  /// Emit all chunks in a chunk graph.
  pub fn emit_all(
    &self,
    chunk_graph: &mut ChunkGraph,
  ) -> Result<Vec<EmittedChunk>, AnyError> {
    let mut results = Vec::new();

    // Collect chunk IDs first to avoid borrow issues
    let chunk_ids: Vec<ChunkId> =
      chunk_graph.chunks().map(|c| c.id.clone()).collect();

    let environment = chunk_graph.environment.clone();

    for chunk_id in chunk_ids {
      let chunk = chunk_graph.get_chunk(&chunk_id).unwrap();
      let emitted = self.emit_chunk_for_env(chunk, &environment)?;

      // Update the chunk with generated code
      if let Some(chunk) = chunk_graph.get_chunk_mut(&chunk_id) {
        chunk.code = Some(emitted.code.clone());
        chunk.source_map = emitted.source_map.clone();
      }

      results.push(emitted);
    }

    Ok(results)
  }

  /// Emit a single chunk.
  pub fn emit_chunk(&self, chunk: &Chunk) -> Result<EmittedChunk, AnyError> {
    self.emit_chunk_for_env(chunk, &BundleEnvironment::Server)
  }

  /// Emit a single chunk for a specific environment.
  pub fn emit_chunk_for_env(
    &self,
    chunk: &Chunk,
    environment: &BundleEnvironment,
  ) -> Result<EmittedChunk, AnyError> {
    let source = self.source_graph.read();

    // Determine module order within the chunk
    let ordered_modules = determine_bundle_order(chunk, self.source_graph);

    // Build the bundle content
    let mut bundle_code = String::new();
    let mut module_map: HashMap<String, String> = HashMap::new();

    // Add environment shim at the start of entry chunks
    if chunk.is_entry {
      bundle_code.push_str(&self.generate_env_shim(environment));
      bundle_code.push('\n');

      // Add HMR runtime preamble in development mode with HMR enabled
      if self.config.hmr && self.config.mode == BuildMode::Development {
        bundle_code.push_str(&self.generate_hmr_preamble(environment));
        bundle_code.push('\n');
      }
    }

    // Check if HMR is enabled for module wrapping
    let use_hmr = self.config.hmr && self.config.mode == BuildMode::Development;

    // Generate module wrapper for each module
    for (idx, specifier) in ordered_modules.iter().enumerate() {
      let module_id = format!("__module_{}__", idx);
      module_map.insert(specifier.to_string(), module_id.clone());

      if let Some(module) = source.get_module(specifier) {
        // Get the transformed or original code
        let code = if let Some(transformed) = &module.transformed {
          transformed.code.to_string()
        } else {
          module.source.to_string()
        };

        // Transpile if needed (TypeScript -> JavaScript)
        let js_code = self.transpile_module(specifier, &code)?;

        // Wrap in module scope (with HMR support if enabled)
        let wrapped = if use_hmr {
          self.wrap_module_with_hmr(&module_id, specifier, &js_code)
        } else {
          self.wrap_module(&module_id, specifier, &js_code)
        };
        bundle_code.push_str(&wrapped);
        bundle_code.push('\n');
      }
    }

    // Add module registry and initialization
    let init_code =
      self.generate_init_code(&ordered_modules, &module_map, chunk);
    bundle_code.push_str(&init_code);

    // Generate source map if enabled
    let source_map = if self.config.source_maps {
      // TODO: Implement proper source map composition
      None
    } else {
      None
    };

    Ok(EmittedChunk {
      chunk_id: chunk.id.clone(),
      file_name: chunk.file_name.clone(),
      code: bundle_code,
      source_map,
    })
  }

  /// Transpile a module from TypeScript to JavaScript.
  fn transpile_module(
    &self,
    specifier: &ModuleSpecifier,
    code: &str,
  ) -> Result<String, AnyError> {
    let media_type = deno_ast::MediaType::from_specifier(specifier);

    // If already JavaScript, return as-is
    if matches!(
      media_type,
      deno_ast::MediaType::JavaScript | deno_ast::MediaType::Mjs
    ) {
      return Ok(code.to_string());
    }

    // Parse the module
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: specifier.clone(),
      text: code.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })?;

    // Transpile to JavaScript
    let emit_options = EmitOptions {
      source_map: if self.config.source_maps {
        SourceMapOption::Separate
      } else {
        SourceMapOption::None
      },
      ..Default::default()
    };

    let transpile_options = TranspileOptions {
      ..Default::default()
    };

    let transpile_module_options = TranspileModuleOptions::default();

    let emitted = parsed.transpile(
      &transpile_options,
      &transpile_module_options,
      &emit_options,
    )?;

    Ok(emitted.into_source().text)
  }

  /// Wrap a module in a scope function.
  fn wrap_module(
    &self,
    module_id: &str,
    specifier: &ModuleSpecifier,
    code: &str,
  ) -> String {
    // Create a module wrapper that exports to a module object
    format!(
      r#"// Module: {}
var {} = (function(exports, module) {{
{}
return module.exports;
}})(Object.create(null), {{ exports: Object.create(null) }});
"#,
      specifier,
      module_id,
      indent_code(code, 2)
    )
  }

  /// Wrap a module in a scope function with HMR support.
  ///
  /// This injects the `import.meta.hot` context for HMR-enabled modules.
  fn wrap_module_with_hmr(
    &self,
    module_id: &str,
    specifier: &ModuleSpecifier,
    code: &str,
  ) -> String {
    hmr_runtime::generate_module_hmr_wrapper(
      &specifier.to_string(),
      module_id,
      code,
    )
  }

  /// Generate environment shim for import.meta.env support.
  fn generate_env_shim(&self, environment: &BundleEnvironment) -> String {
    let mode = match self.config.mode {
      BuildMode::Development => "development",
      BuildMode::Production => "production",
    };

    let is_dev = matches!(self.config.mode, BuildMode::Development);
    let is_prod = matches!(self.config.mode, BuildMode::Production);
    let is_ssr = environment.is_server();

    // Build env vars object
    let mut env_entries = vec![
      format!("  MODE: \"{}\",", mode),
      format!("  DEV: {},", is_dev),
      format!("  PROD: {},", is_prod),
      format!("  SSR: {},", is_ssr),
    ];

    // Add custom env vars
    for (key, value) in &self.config.env_vars {
      // Escape the value for JavaScript string
      let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
      env_entries.push(format!("  {}: \"{}\",", key, escaped));
    }

    format!(
      r#"// Environment shim for import.meta.env
var __env__ = {{
{}
}};
if (typeof globalThis !== "undefined") {{
  globalThis.__VBUNDLE_ENV__ = __env__;
}}
"#,
      env_entries.join("\n")
    )
  }

  /// Generate HMR runtime preamble for injection into entry chunks.
  ///
  /// This creates the `__VBUNDLE_HMR__` global object with the full HMR API.
  fn generate_hmr_preamble(&self, environment: &BundleEnvironment) -> String {
    let config = self.config.hmr_config.clone().unwrap_or_default();
    hmr_runtime::generate_hmr_runtime(&config, environment)
  }

  /// Generate initialization code for the bundle.
  fn generate_init_code(
    &self,
    ordered_modules: &[ModuleSpecifier],
    module_map: &HashMap<String, String>,
    chunk: &Chunk,
  ) -> String {
    let mut init_code = String::new();

    // For entry chunks, execute the entry module
    if chunk.is_entry && !ordered_modules.is_empty() {
      let entry_specifier = &ordered_modules[ordered_modules.len() - 1];
      if let Some(module_id) = module_map.get(&entry_specifier.to_string()) {
        init_code.push_str(&format!(
          "\n// Entry point\nvar __entry__ = {};\n",
          module_id
        ));
      }
    }

    // Export chunk exports if any
    if !chunk.exports.is_empty() {
      init_code.push_str("\n// Exports\n");
      for export_name in &chunk.exports {
        init_code.push_str(&format!("export {{ {} }};\n", export_name));
      }
    }

    init_code
  }

  /// Write emitted chunks to disk.
  pub fn write_to_disk(
    &self,
    emitted: &[EmittedChunk],
  ) -> Result<(), AnyError> {
    std::fs::create_dir_all(&self.config.out_dir)?;

    for chunk in emitted {
      let file_path = self.config.out_dir.join(&chunk.file_name);
      std::fs::write(&file_path, &chunk.code)?;

      if let Some(source_map) = &chunk.source_map {
        let map_path = file_path.with_extension("js.map");
        std::fs::write(&map_path, source_map)?;
      }
    }

    Ok(())
  }
}

/// Indent code by a number of spaces.
fn indent_code(code: &str, spaces: usize) -> String {
  let indent = " ".repeat(spaces);
  code
    .lines()
    .map(|line| {
      if line.is_empty() {
        line.to_string()
      } else {
        format!("{}{}", indent, line)
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// Generate a module import rewriter.
///
/// This transforms import statements to reference bundled modules.
pub fn rewrite_imports(
  code: &str,
  specifier: &ModuleSpecifier,
  module_map: &HashMap<String, String>,
) -> Result<String, AnyError> {
  // For now, return unchanged - full implementation would use SWC visitor
  // to rewrite import/export statements
  // TODO: Implement import rewriting with SWC
  Ok(code.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_indent_code() {
    let code = "const x = 1;\nconst y = 2;";
    let indented = indent_code(code, 2);
    assert_eq!(indented, "  const x = 1;\n  const y = 2;");
  }

  #[test]
  fn test_wrap_module() {
    let source_graph = SharedSourceGraph::new();
    let emitter = ChunkEmitter::new(&source_graph, EmitterConfig::default());

    let specifier = ModuleSpecifier::parse("file:///app/mod.ts").unwrap();
    let wrapped =
      emitter.wrap_module("__module_0__", &specifier, "export const x = 1;");

    assert!(wrapped.contains("__module_0__"));
    assert!(wrapped.contains("export const x = 1;"));
  }

  #[test]
  fn test_build_mode_default() {
    let config = EmitterConfig::default();
    assert!(matches!(config.mode, BuildMode::Development));
    assert!(!config.minify);
  }

  #[test]
  fn test_build_mode_production() {
    let config = EmitterConfig::production();
    assert!(matches!(config.mode, BuildMode::Production));
    assert!(config.minify);
  }

  #[test]
  fn test_emitter_config_with_env_vars() {
    let config = EmitterConfig::default()
      .with_env_var("API_URL", "https://api.example.com")
      .with_env_var("DEBUG", "true");

    assert_eq!(
      config.env_vars.get("API_URL"),
      Some(&"https://api.example.com".to_string())
    );
    assert_eq!(config.env_vars.get("DEBUG"), Some(&"true".to_string()));
  }

  #[test]
  fn test_generate_env_shim_development_server() {
    let source_graph = SharedSourceGraph::new();
    let emitter = ChunkEmitter::new(&source_graph, EmitterConfig::default());

    let shim = emitter.generate_env_shim(&BundleEnvironment::Server);

    assert!(shim.contains("MODE: \"development\""));
    assert!(shim.contains("DEV: true"));
    assert!(shim.contains("PROD: false"));
    assert!(shim.contains("SSR: true"));
  }

  #[test]
  fn test_generate_env_shim_development_browser() {
    let source_graph = SharedSourceGraph::new();
    let emitter = ChunkEmitter::new(&source_graph, EmitterConfig::default());

    let shim = emitter.generate_env_shim(&BundleEnvironment::Browser);

    assert!(shim.contains("MODE: \"development\""));
    assert!(shim.contains("DEV: true"));
    assert!(shim.contains("PROD: false"));
    assert!(shim.contains("SSR: false"));
  }

  #[test]
  fn test_generate_env_shim_production() {
    let source_graph = SharedSourceGraph::new();
    let config = EmitterConfig::production();
    let emitter = ChunkEmitter::new(&source_graph, config);

    let shim = emitter.generate_env_shim(&BundleEnvironment::Server);

    assert!(shim.contains("MODE: \"production\""));
    assert!(shim.contains("DEV: false"));
    assert!(shim.contains("PROD: true"));
  }

  #[test]
  fn test_generate_env_shim_with_custom_vars() {
    let source_graph = SharedSourceGraph::new();
    let config = EmitterConfig::default()
      .with_env_var("API_URL", "https://api.example.com")
      .with_env_var("VERSION", "1.0.0");
    let emitter = ChunkEmitter::new(&source_graph, config);

    let shim = emitter.generate_env_shim(&BundleEnvironment::Server);

    assert!(shim.contains("API_URL: \"https://api.example.com\""));
    assert!(shim.contains("VERSION: \"1.0.0\""));
  }

  #[test]
  fn test_generate_env_shim_escapes_special_chars() {
    let source_graph = SharedSourceGraph::new();
    let config = EmitterConfig::default()
      .with_env_var("MESSAGE", "Hello \"World\"\nNew line");
    let emitter = ChunkEmitter::new(&source_graph, config);

    let shim = emitter.generate_env_shim(&BundleEnvironment::Server);

    // Should escape quotes and newlines
    assert!(shim.contains(r#"MESSAGE: "Hello \"World\"\nNew line""#));
  }

  #[test]
  fn test_emitter_config_with_hmr() {
    let config = EmitterConfig::default().with_hmr();

    assert!(config.hmr);
    assert!(config.hmr_config.is_some());
  }

  #[test]
  fn test_emitter_config_production_no_hmr() {
    let config = EmitterConfig::production();

    // HMR should be disabled in production
    assert!(!config.hmr);
    assert!(config.hmr_config.is_none());
  }

  #[test]
  fn test_generate_hmr_preamble_browser() {
    let source_graph = SharedSourceGraph::new();
    let config = EmitterConfig::default().with_hmr();
    let emitter = ChunkEmitter::new(&source_graph, config);

    let preamble = emitter.generate_hmr_preamble(&BundleEnvironment::Browser);

    // Should contain HMR runtime markers
    assert!(preamble.contains("__VBUNDLE_HMR__"));
    assert!(preamble.contains("ViteHotContext"));
    assert!(preamble.contains("createHotContext"));
    assert!(preamble.contains("applyUpdate"));
    // Browser should have WebSocket and location.reload
    assert!(preamble.contains("WebSocket"));
    assert!(preamble.contains("location.reload"));
  }

  #[test]
  fn test_generate_hmr_preamble_server() {
    let source_graph = SharedSourceGraph::new();
    let config = EmitterConfig::default().with_hmr();
    let emitter = ChunkEmitter::new(&source_graph, config);

    let preamble = emitter.generate_hmr_preamble(&BundleEnvironment::Server);

    // Should contain HMR runtime markers
    assert!(preamble.contains("__VBUNDLE_HMR__"));
    assert!(preamble.contains("ViteHotContext"));
    // Server should not use location.reload
    assert!(!preamble.contains("location.reload()"));
    assert!(preamble.contains("please restart the process"));
  }

  #[test]
  fn test_wrap_module_with_hmr() {
    let source_graph = SharedSourceGraph::new();
    let config = EmitterConfig::default().with_hmr();
    let emitter = ChunkEmitter::new(&source_graph, config);

    let specifier = ModuleSpecifier::parse("file:///app/mod.ts").unwrap();
    let wrapped = emitter.wrap_module_with_hmr(
      "__module_0__",
      &specifier,
      "export const x = 1;",
    );

    // Should contain HMR context creation
    assert!(wrapped.contains("__module_0__"));
    assert!(wrapped.contains("import.meta.hot"));
    assert!(wrapped.contains("__VBUNDLE_HMR__.createHotContext"));
    assert!(wrapped.contains("file:///app/mod.ts"));
  }
}
