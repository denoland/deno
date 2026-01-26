// Copyright 2018-2026 the Deno authors. MIT license.

//! Virtual File System for the bundler.
//!
//! The VFS is the core abstraction enabling integration with all Deno tools.
//! It provides a unified interface for:
//! - Lazy transformation of non-JS files (e.g., .svelte, .vue)
//! - Caching of transformed results
//! - Source map handling for error position mapping
//!
//! The VFS can operate in two modes:
//! - **Lazy mode**: Transform files on-demand (for `run`/`test`/`lint`/`check`)
//! - **Eager mode**: Bundle all files upfront (for `bundle`/`compile`)

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::parking_lot::RwLock;

use super::plugins::PluginHostProxy;
use super::source_map::Position;
use super::source_map::SourceMapCache;
use super::source_map::SourceMapInfo;
use super::source_map::SourceRange;
use super::types::TransformedModule;

/// A cached entry in the VFS.
#[derive(Debug, Clone)]
pub struct VfsCacheEntry {
  /// The transformed module.
  pub module: TransformedModule,
  /// Timestamp when this entry was cached.
  pub cached_at: std::time::Instant,
  /// Hash of the original source (for invalidation).
  pub source_hash: u64,
}

impl VfsCacheEntry {
  pub fn new(module: TransformedModule, source_hash: u64) -> Self {
    Self {
      module,
      cached_at: std::time::Instant::now(),
      source_hash,
    }
  }
}

/// Configuration for the VFS.
#[derive(Debug, Clone)]
pub struct VfsConfig {
  /// Whether to enable caching.
  pub enable_cache: bool,
  /// Maximum cache entries (0 = unlimited).
  pub max_cache_entries: usize,
  /// Whether to generate source maps.
  pub source_maps: bool,
}

impl Default for VfsConfig {
  fn default() -> Self {
    Self {
      enable_cache: true,
      max_cache_entries: 0,
      source_maps: true,
    }
  }
}

/// The Bundler Virtual File System.
///
/// This provides a transparent transformation layer between the file system
/// and all Deno tools. When a tool requests a file:
///
/// 1. Check if it's in cache
/// 2. If not cached, check if any plugin handles this file type
/// 3. If a plugin handles it, call load/transform hooks
/// 4. Cache the result and return transformed code
/// 5. If no plugin handles it, return the original file
pub struct BundlerVirtualFS {
  /// Plugin host for on-demand transformation.
  plugin_host: Option<Arc<PluginHostProxy>>,

  /// Cache of transformed files: specifier → cached entry.
  cache: DashMap<ModuleSpecifier, VfsCacheEntry>,

  /// Source maps for error position mapping.
  source_maps: RwLock<SourceMapCache>,

  /// File extensions handled by plugins: extension → plugin name.
  extension_handlers: RwLock<HashMap<String, String>>,

  /// VFS configuration.
  config: VfsConfig,
}

impl BundlerVirtualFS {
  /// Create a new VFS with the given plugin host.
  pub fn new(
    plugin_host: Option<Arc<PluginHostProxy>>,
    config: VfsConfig,
  ) -> Self {
    Self {
      plugin_host,
      cache: DashMap::new(),
      source_maps: RwLock::new(SourceMapCache::new()),
      extension_handlers: RwLock::new(HashMap::new()),
      config,
    }
  }

  /// Create a VFS without plugins (pass-through mode).
  pub fn passthrough() -> Self {
    Self::new(None, VfsConfig::default())
  }

  /// Register file extensions handled by plugins.
  pub fn register_extensions(&self, extensions: &[String], plugin_name: &str) {
    let mut handlers = self.extension_handlers.write();
    for ext in extensions {
      let ext = ext.trim_start_matches('.').to_lowercase();
      handlers.insert(ext, plugin_name.to_string());
    }
  }

  /// Check if any plugin handles the given file extension.
  pub fn handles_extension(&self, ext: &str) -> bool {
    let ext = ext.trim_start_matches('.').to_lowercase();
    let handlers = self.extension_handlers.read();
    handlers.contains_key(&ext)
  }

  /// Get the extension of a specifier.
  fn get_extension(specifier: &ModuleSpecifier) -> Option<String> {
    let path = specifier.path();
    // Get the last path segment (filename)
    let filename = path.rsplit('/').next()?;
    // Get the extension (part after the last dot)
    if let Some(dot_pos) = filename.rfind('.') {
      let ext = &filename[dot_pos + 1..];
      if !ext.is_empty() {
        return Some(ext.to_lowercase());
      }
    }
    None
  }

  /// Check if a specifier needs transformation.
  pub fn needs_transform(&self, specifier: &ModuleSpecifier) -> bool {
    if let Some(ext) = Self::get_extension(specifier) {
      self.handles_extension(&ext)
    } else {
      false
    }
  }

  /// Load a module, transforming it if necessary.
  ///
  /// This is the main entry point for the VFS. It handles:
  /// - Cache lookup
  /// - Plugin load/transform hooks
  /// - Source map generation
  /// - Caching the result
  pub async fn load(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<TransformedModule, AnyError> {
    // Check cache first
    if self.config.enable_cache {
      if let Some(entry) = self.cache.get(specifier) {
        return Ok(entry.module.clone());
      }
    }

    // Check if we need to transform this file
    let needs_transform = self.needs_transform(specifier);

    if needs_transform {
      if let Some(host) = &self.plugin_host {
        return self.load_with_plugins(specifier, host).await;
      }
    }

    // No transformation needed - load as passthrough
    self.load_passthrough(specifier).await
  }

  /// Load a module using plugins.
  async fn load_with_plugins(
    &self,
    specifier: &ModuleSpecifier,
    host: &PluginHostProxy,
  ) -> Result<TransformedModule, AnyError> {
    let id = specifier.as_str();

    // Try plugin load hook first
    let (code, media_type) = if let Some(load_result) = host.load(id).await? {
      let media_type = match load_result.loader.as_deref() {
        Some("ts") | Some("typescript") => MediaType::TypeScript,
        Some("tsx") => MediaType::Tsx,
        Some("jsx") => MediaType::Jsx,
        Some("json") => MediaType::Json,
        _ => MediaType::JavaScript,
      };
      (load_result.code, media_type)
    } else {
      // Plugin didn't handle load, load natively
      let source = self.load_native(specifier).await?;
      let media_type = MediaType::from_specifier(specifier);
      (source, media_type)
    };

    // Try plugin transform hook
    let (final_code, source_map_json) =
      if let Some(transform_result) = host.transform(id, &code).await? {
        (transform_result.code, transform_result.map)
      } else {
        (code, None)
      };

    // Parse and store source map
    let source_map = if let Some(map_json) = source_map_json {
      match SourceMapInfo::from_json(&map_json, specifier.clone()) {
        Ok(info) => {
          self
            .source_maps
            .write()
            .insert(specifier.clone(), info.clone());
          Some(info.source_map().clone())
        }
        Err(e) => {
          log::warn!("Failed to parse source map for {}: {}", specifier, e);
          None
        }
      }
    } else {
      None
    };

    let module = TransformedModule {
      original_specifier: specifier.clone(),
      code: final_code.into(),
      source_map,
      media_type: MediaType::JavaScript, // After transformation, always JS
      declarations: None,
    };

    // Cache the result
    if self.config.enable_cache {
      let hash = self.hash_source(&module.code);
      self
        .cache
        .insert(specifier.clone(), VfsCacheEntry::new(module.clone(), hash));
    }

    Ok(module)
  }

  /// Load a module without transformation (passthrough).
  async fn load_passthrough(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<TransformedModule, AnyError> {
    let source = self.load_native(specifier).await?;
    let media_type = MediaType::from_specifier(specifier);

    Ok(TransformedModule {
      original_specifier: specifier.clone(),
      code: source.into(),
      source_map: None,
      media_type,
      declarations: None,
    })
  }

  /// Load source code from the file system or network.
  async fn load_native(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<String, AnyError> {
    if specifier.scheme() == "file" {
      let path = specifier.to_file_path().map_err(|_| {
        deno_core::anyhow::anyhow!("Invalid file URL: {}", specifier)
      })?;
      let content = tokio::fs::read_to_string(&path).await?;
      Ok(content)
    } else {
      // For remote modules, we would use the module fetcher
      // For now, return an error
      Err(deno_core::anyhow::anyhow!(
        "Remote modules not yet supported in VFS: {}",
        specifier
      ))
    }
  }

  /// Simple hash function for cache invalidation.
  fn hash_source(&self, source: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;

    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
  }

  /// Map an error position from transformed code to original source.
  pub fn map_error_position(
    &self,
    specifier: &ModuleSpecifier,
    pos: Position,
  ) -> Position {
    self.source_maps.read().map_position(specifier, pos)
  }

  /// Map an error range from transformed code to original source.
  pub fn map_error_range(
    &self,
    specifier: &ModuleSpecifier,
    range: SourceRange,
  ) -> SourceRange {
    self.source_maps.read().map_range(specifier, range)
  }

  /// Get the original source for a specifier (from source map if available).
  pub fn get_original_source(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    let maps = self.source_maps.read();
    if let Some(info) = maps.get(specifier) {
      info.original_source().map(|s| s.to_string())
    } else {
      None
    }
  }

  /// Check if a specifier is cached.
  pub fn is_cached(&self, specifier: &ModuleSpecifier) -> bool {
    self.cache.contains_key(specifier)
  }

  /// Invalidate cache for a specifier.
  pub fn invalidate(&self, specifier: &ModuleSpecifier) {
    self.cache.remove(specifier);
    self.source_maps.write().remove(specifier);
  }

  /// Clear all caches.
  pub fn clear_cache(&self) {
    self.cache.clear();
    self.source_maps.write().clear();
  }

  /// Get cache statistics.
  pub fn cache_stats(&self) -> CacheStats {
    CacheStats {
      entries: self.cache.len(),
      source_maps: self.source_maps.read().len(),
    }
  }

  /// Get the number of registered extension handlers.
  pub fn extension_handler_count(&self) -> usize {
    self.extension_handlers.read().len()
  }

  /// Synchronous load for compatibility with sync code paths.
  ///
  /// This blocks on the async load. Use sparingly.
  pub fn load_sync(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<TransformedModule, AnyError> {
    // Check cache first (no async needed)
    if self.config.enable_cache {
      if let Some(entry) = self.cache.get(specifier) {
        return Ok(entry.module.clone());
      }
    }

    // For sync loading, we only support passthrough (no plugin calls)
    if self.needs_transform(specifier) {
      return Err(deno_core::anyhow::anyhow!(
        "Cannot synchronously load file that needs transformation: {}",
        specifier
      ));
    }

    // Load synchronously from file system
    if specifier.scheme() == "file" {
      let path = specifier.to_file_path().map_err(|_| {
        deno_core::anyhow::anyhow!("Invalid file URL: {}", specifier)
      })?;
      let content = std::fs::read_to_string(&path)?;
      let media_type = MediaType::from_specifier(specifier);

      Ok(TransformedModule {
        original_specifier: specifier.clone(),
        code: content.into(),
        source_map: None,
        media_type,
        declarations: None,
      })
    } else {
      Err(deno_core::anyhow::anyhow!(
        "Cannot synchronously load remote module: {}",
        specifier
      ))
    }
  }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
  pub entries: usize,
  pub source_maps: usize,
}

impl std::fmt::Debug for BundlerVirtualFS {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BundlerVirtualFS")
      .field("has_plugin_host", &self.plugin_host.is_some())
      .field("cache_entries", &self.cache.len())
      .field("config", &self.config)
      .finish()
  }
}

/// Builder for BundlerVirtualFS.
#[derive(Default)]
pub struct VfsBuilder {
  plugin_host: Option<Arc<PluginHostProxy>>,
  config: VfsConfig,
  extensions: Vec<(String, Vec<String>)>,
}

impl VfsBuilder {
  pub fn new() -> Self {
    Self::default()
  }

  /// Set the plugin host.
  pub fn plugin_host(mut self, host: Arc<PluginHostProxy>) -> Self {
    self.plugin_host = Some(host);
    self
  }

  /// Set VFS configuration.
  pub fn config(mut self, config: VfsConfig) -> Self {
    self.config = config;
    self
  }

  /// Enable or disable caching.
  pub fn enable_cache(mut self, enable: bool) -> Self {
    self.config.enable_cache = enable;
    self
  }

  /// Enable or disable source maps.
  pub fn source_maps(mut self, enable: bool) -> Self {
    self.config.source_maps = enable;
    self
  }

  /// Register extensions for a plugin.
  pub fn register_extensions(
    mut self,
    plugin_name: &str,
    extensions: Vec<String>,
  ) -> Self {
    self.extensions.push((plugin_name.to_string(), extensions));
    self
  }

  /// Build the VFS.
  pub fn build(self) -> BundlerVirtualFS {
    let vfs = BundlerVirtualFS::new(self.plugin_host, self.config);

    for (plugin_name, extensions) in self.extensions {
      vfs.register_extensions(&extensions, &plugin_name);
    }

    vfs
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_extension() {
    let spec = ModuleSpecifier::parse("file:///test.svelte").unwrap();
    assert_eq!(
      BundlerVirtualFS::get_extension(&spec),
      Some("svelte".to_string())
    );

    let spec = ModuleSpecifier::parse("file:///test.vue").unwrap();
    assert_eq!(
      BundlerVirtualFS::get_extension(&spec),
      Some("vue".to_string())
    );

    // Files without an extension should return None
    let spec = ModuleSpecifier::parse("file:///test").unwrap();
    assert_eq!(BundlerVirtualFS::get_extension(&spec), None);

    // Nested paths
    let spec =
      ModuleSpecifier::parse("file:///src/components/App.svelte").unwrap();
    assert_eq!(
      BundlerVirtualFS::get_extension(&spec),
      Some("svelte".to_string())
    );

    // Multiple dots
    let spec = ModuleSpecifier::parse("file:///test.spec.ts").unwrap();
    assert_eq!(
      BundlerVirtualFS::get_extension(&spec),
      Some("ts".to_string())
    );
  }

  #[test]
  fn test_handles_extension() {
    let vfs = VfsBuilder::new()
      .register_extensions("svelte", vec![".svelte".to_string()])
      .build();

    assert!(vfs.handles_extension("svelte"));
    assert!(vfs.handles_extension(".svelte"));
    assert!(vfs.handles_extension("SVELTE"));
    assert!(!vfs.handles_extension("vue"));
  }

  #[test]
  fn test_needs_transform() {
    let vfs = VfsBuilder::new()
      .register_extensions("svelte", vec![".svelte".to_string()])
      .build();

    let spec = ModuleSpecifier::parse("file:///app.svelte").unwrap();
    assert!(vfs.needs_transform(&spec));

    let spec = ModuleSpecifier::parse("file:///app.ts").unwrap();
    assert!(!vfs.needs_transform(&spec));
  }
}
