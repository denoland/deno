// Copyright 2018-2026 the Deno authors. MIT license.

//! Plugin system for the bundler.
//!
//! Follows an esbuild-style plugin API with `onResolve`, `onLoad`,
//! `onTransform`, and `onWatchChange` hooks. Hooks are matched by regex
//! filter and optional namespace.
//!
//! The built-in TypeScript transpiler is registered as a normal-order
//! transform hook, making transpilation part of the same chain as plugins.

use std::path::Path;
use std::path::PathBuf;

use regex::Regex;

use crate::loader::Loader;

// ---------------------------------------------------------------------------
// Hook filter
// ---------------------------------------------------------------------------

/// Filter that determines whether a hook should run for a given module.
pub struct HookFilter {
  /// Regex pattern. For `onResolve`, matches the specifier.
  /// For `onLoad`/`onTransform`, matches the file path.
  pub filter: Regex,
  /// Optional namespace constraint. If set, only modules in this namespace
  /// match.
  pub namespace: Option<String>,
}

impl HookFilter {
  pub fn new(filter: Regex) -> Self {
    Self {
      filter,
      namespace: None,
    }
  }

  pub fn with_namespace(filter: Regex, namespace: String) -> Self {
    Self {
      filter,
      namespace: Some(namespace),
    }
  }

  pub fn matches(&self, path_or_specifier: &str, namespace: &str) -> bool {
    if let Some(ns) = &self.namespace {
      if ns != namespace {
        return false;
      }
    }
    self.filter.is_match(path_or_specifier)
  }
}

// ---------------------------------------------------------------------------
// Hook ordering
// ---------------------------------------------------------------------------

/// Ordering for hook execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PluginOrder {
  /// Runs before normal hooks.
  Pre = 0,
  /// Default ordering.
  Normal = 1,
  /// Runs after normal hooks.
  Post = 2,
}

// ---------------------------------------------------------------------------
// Resolve hook
// ---------------------------------------------------------------------------

/// Arguments passed to `onResolve` hooks.
pub struct ResolveArgs<'a> {
  /// The import specifier as written in source code.
  pub specifier: &'a str,
  /// Absolute path of the importing module.
  pub importer: &'a Path,
  /// Namespace of the importing module.
  pub namespace: &'a str,
  /// The type of import (static, dynamic, require, etc.).
  pub kind: ResolveKind,
}

/// The type of import that triggered resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveKind {
  /// Static ESM `import`.
  Import,
  /// Dynamic `import()`.
  DynamicImport,
  /// CommonJS `require()`.
  Require,
  /// CSS `@import`.
  CssImport,
  /// CSS `url()`.
  CssUrl,
  /// HTML asset reference.
  HtmlAsset,
  /// Entry point.
  Entry,
}

/// Result returned from an `onResolve` hook.
pub struct ResolveResult {
  /// The resolved absolute path.
  pub path: PathBuf,
  /// Namespace for the resolved module (default: "file").
  pub namespace: String,
  /// Whether this module should be excluded from bundling.
  pub external: bool,
}

impl ResolveResult {
  pub fn new(path: PathBuf) -> Self {
    Self {
      path,
      namespace: "file".to_string(),
      external: false,
    }
  }

  pub fn with_namespace(path: PathBuf, namespace: String) -> Self {
    Self {
      path,
      namespace,
      external: false,
    }
  }

  pub fn external(path: PathBuf) -> Self {
    Self {
      path,
      namespace: "file".to_string(),
      external: true,
    }
  }
}

/// Trait for resolve hooks.
pub trait OnResolve: Send + Sync {
  fn on_resolve(&self, args: &ResolveArgs) -> Option<ResolveResult>;
}

// ---------------------------------------------------------------------------
// Load hook
// ---------------------------------------------------------------------------

/// Arguments passed to `onLoad` hooks.
pub struct LoadArgs<'a> {
  /// Absolute path of the module to load.
  pub path: &'a Path,
  /// Namespace of the module.
  pub namespace: &'a str,
}

/// Result returned from an `onLoad` hook.
pub struct LoadResult {
  /// Text content of the module.
  pub content: String,
  /// How the module should be parsed.
  pub loader: Loader,
  /// Optional binary content (for assets/binary modules).
  /// When set, `content` may be empty.
  pub asset_bytes: Option<Vec<u8>>,
}

/// Trait for load hooks.
pub trait OnLoad: Send + Sync {
  fn on_load(&self, args: &LoadArgs) -> Option<LoadResult>;
}

// ---------------------------------------------------------------------------
// Transform hook
// ---------------------------------------------------------------------------

/// Arguments passed to `onTransform` hooks.
pub struct TransformArgs<'a> {
  /// The module's text content.
  pub content: &'a str,
  /// Absolute path of the module.
  pub path: &'a Path,
  /// Namespace of the module.
  pub namespace: &'a str,
  /// Current loader type.
  pub loader: Loader,
  /// Source map from previous transform (v3 JSON string).
  pub source_map: Option<&'a str>,
}

/// Result returned from an `onTransform` hook.
pub struct TransformResult {
  /// Transformed content. `None` means no change.
  pub content: Option<String>,
  /// New loader type. `None` means no change.
  /// If changed, the transform pipeline re-runs with the new loader.
  pub loader: Option<Loader>,
  /// Source map for this transform (v3 JSON string).
  pub source_map: Option<String>,
}

/// Trait for transform hooks.
pub trait OnTransform: Send + Sync {
  fn on_transform(&self, args: &TransformArgs) -> Option<TransformResult>;
}

// ---------------------------------------------------------------------------
// Watch change hook
// ---------------------------------------------------------------------------

/// Arguments passed to `onWatchChange` hooks.
pub struct WatchChangeArgs<'a> {
  /// Path of the changed file.
  pub path: &'a Path,
}

/// Result returned from an `onWatchChange` hook.
#[derive(Default)]
pub struct WatchChangeResult {
  /// Entry points to add.
  pub add_entries: Vec<String>,
  /// Entry points to remove.
  pub remove_entries: Vec<String>,
}

/// Trait for watch change hooks.
pub trait OnWatchChange: Send + Sync {
  fn on_watch_change(
    &self,
    args: &WatchChangeArgs,
  ) -> Option<WatchChangeResult>;
}

// ---------------------------------------------------------------------------
// Plugin trait
// ---------------------------------------------------------------------------

/// A bundler plugin.
pub trait Plugin: Send + Sync {
  /// Plugin name, used for logging and error messages.
  fn name(&self) -> &str;

  /// Register hooks during setup.
  fn setup(&self, build: &mut PluginBuild);
}

// ---------------------------------------------------------------------------
// PluginBuild — hook registration during setup
// ---------------------------------------------------------------------------

/// Handle passed to `Plugin::setup()` for registering hooks.
pub struct PluginBuild {
  pub(crate) resolve_hooks: Vec<(HookFilter, PluginOrder, Box<dyn OnResolve>)>,
  pub(crate) load_hooks: Vec<(HookFilter, PluginOrder, Box<dyn OnLoad>)>,
  pub(crate) transform_hooks:
    Vec<(HookFilter, PluginOrder, Box<dyn OnTransform>)>,
  pub(crate) watch_change_hooks:
    Vec<(HookFilter, PluginOrder, Box<dyn OnWatchChange>)>,
}

impl PluginBuild {
  pub fn new() -> Self {
    Self {
      resolve_hooks: Vec::new(),
      load_hooks: Vec::new(),
      transform_hooks: Vec::new(),
      watch_change_hooks: Vec::new(),
    }
  }

  pub fn on_resolve(
    &mut self,
    filter: HookFilter,
    hook: Box<dyn OnResolve>,
  ) {
    self
      .resolve_hooks
      .push((filter, PluginOrder::Normal, hook));
  }

  pub fn on_resolve_with_order(
    &mut self,
    filter: HookFilter,
    order: PluginOrder,
    hook: Box<dyn OnResolve>,
  ) {
    self.resolve_hooks.push((filter, order, hook));
  }

  pub fn on_load(&mut self, filter: HookFilter, hook: Box<dyn OnLoad>) {
    self.load_hooks.push((filter, PluginOrder::Normal, hook));
  }

  pub fn on_load_with_order(
    &mut self,
    filter: HookFilter,
    order: PluginOrder,
    hook: Box<dyn OnLoad>,
  ) {
    self.load_hooks.push((filter, order, hook));
  }

  pub fn on_transform(
    &mut self,
    filter: HookFilter,
    hook: Box<dyn OnTransform>,
  ) {
    self
      .transform_hooks
      .push((filter, PluginOrder::Normal, hook));
  }

  pub fn on_transform_with_order(
    &mut self,
    filter: HookFilter,
    order: PluginOrder,
    hook: Box<dyn OnTransform>,
  ) {
    self.transform_hooks.push((filter, order, hook));
  }

  pub fn on_watch_change(
    &mut self,
    filter: HookFilter,
    hook: Box<dyn OnWatchChange>,
  ) {
    self
      .watch_change_hooks
      .push((filter, PluginOrder::Normal, hook));
  }
}

impl Default for PluginBuild {
  fn default() -> Self {
    Self::new()
  }
}

// ---------------------------------------------------------------------------
// PluginDriver — executes hooks
// ---------------------------------------------------------------------------

/// Drives plugin hook execution.
///
/// Manages registered hooks sorted by order. For resolve/load, the first
/// hook that returns `Some` wins. For transform, all matching hooks run
/// sequentially. If a transform changes the loader, the pipeline re-runs
/// (max 10 times).
pub struct PluginDriver {
  resolve_hooks: Vec<(HookFilter, Box<dyn OnResolve>)>,
  load_hooks: Vec<(HookFilter, Box<dyn OnLoad>)>,
  transform_hooks: Vec<(HookFilter, Box<dyn OnTransform>)>,
  watch_change_hooks: Vec<(HookFilter, Box<dyn OnWatchChange>)>,
}

impl PluginDriver {
  /// Create a `PluginDriver` from a set of plugins.
  pub fn new(plugins: Vec<Box<dyn Plugin>>) -> Self {
    let mut build = PluginBuild::new();

    for plugin in &plugins {
      plugin.setup(&mut build);
    }

    Self::from_build(build)
  }

  /// Create a `PluginDriver` with no plugins (only built-in hooks).
  pub fn empty() -> Self {
    Self {
      resolve_hooks: Vec::new(),
      load_hooks: Vec::new(),
      transform_hooks: Vec::new(),
      watch_change_hooks: Vec::new(),
    }
  }

  fn from_build(build: PluginBuild) -> Self {
    // Sort hooks by order (stable sort preserves registration order
    // within the same priority level).
    let mut resolve = build.resolve_hooks;
    resolve.sort_by_key(|(_, order, _)| *order);
    let resolve_hooks = resolve
      .into_iter()
      .map(|(filter, _, hook)| (filter, hook))
      .collect();

    let mut load = build.load_hooks;
    load.sort_by_key(|(_, order, _)| *order);
    let load_hooks = load
      .into_iter()
      .map(|(filter, _, hook)| (filter, hook))
      .collect();

    let mut transform = build.transform_hooks;
    transform.sort_by_key(|(_, order, _)| *order);
    let transform_hooks = transform
      .into_iter()
      .map(|(filter, _, hook)| (filter, hook))
      .collect();

    let mut watch_change = build.watch_change_hooks;
    watch_change.sort_by_key(|(_, order, _)| *order);
    let watch_change_hooks = watch_change
      .into_iter()
      .map(|(filter, _, hook)| (filter, hook))
      .collect();

    Self {
      resolve_hooks,
      load_hooks,
      transform_hooks,
      watch_change_hooks,
    }
  }

  /// Run resolve hooks. First `Some` result wins.
  pub fn resolve(&self, args: &ResolveArgs) -> Option<ResolveResult> {
    for (filter, hook) in &self.resolve_hooks {
      if filter.matches(args.specifier, args.namespace) {
        if let Some(result) = hook.on_resolve(args) {
          return Some(result);
        }
      }
    }
    None
  }

  /// Run load hooks. First `Some` result wins.
  pub fn load(&self, args: &LoadArgs) -> Option<LoadResult> {
    let path_str = args.path.to_string_lossy();
    for (filter, hook) in &self.load_hooks {
      if filter.matches(&path_str, args.namespace) {
        if let Some(result) = hook.on_load(args) {
          return Some(result);
        }
      }
    }
    None
  }

  /// Run all matching transform hooks sequentially.
  ///
  /// If a hook changes the loader, the pipeline re-runs from the start
  /// (excluding the hook that triggered the change) with the new loader.
  /// Maximum 10 re-runs to prevent infinite loops.
  pub fn transform(
    &self,
    mut content: String,
    path: &Path,
    namespace: &str,
    mut loader: Loader,
  ) -> TransformOutput {
    let mut source_map: Option<String> = None;
    let mut runs = 0;
    const MAX_RERUNS: usize = 10;

    // Track which hook indices have triggered a loader change.
    // These are excluded from subsequent re-runs to prevent infinite loops.
    let mut skip_indices: Vec<usize> = Vec::new();

    loop {
      if runs >= MAX_RERUNS {
        eprintln!(
          "Transform pipeline exceeded {} re-runs for {}",
          MAX_RERUNS,
          path.display()
        );
        break;
      }
      runs += 1;

      let path_str = path.to_string_lossy();
      let mut loader_changed = false;

      for (i, (filter, hook)) in self.transform_hooks.iter().enumerate() {
        // Skip hooks that previously triggered a loader change.
        if skip_indices.contains(&i) {
          continue;
        }

        if !filter.matches(&path_str, namespace) {
          continue;
        }

        let args = TransformArgs {
          content: &content,
          path,
          namespace,
          loader,
          source_map: source_map.as_deref(),
        };

        if let Some(result) = hook.on_transform(&args) {
          if let Some(new_content) = result.content {
            content = new_content;
          }
          if result.source_map.is_some() {
            source_map = result.source_map;
          }
          if let Some(new_loader) = result.loader {
            if new_loader != loader {
              loader = new_loader;
              loader_changed = true;
              skip_indices.push(i);
              break; // Re-run the pipeline with the new loader.
            }
          }
        }
      }

      if !loader_changed {
        break; // All hooks ran without changing the loader. Done.
      }
    }

    TransformOutput {
      content,
      loader,
      source_map,
    }
  }

  /// Notify watch change hooks.
  pub fn notify_watch_change(&self, path: &Path) -> WatchChangeResult {
    let path_str = path.to_string_lossy();
    let mut result = WatchChangeResult::default();

    for (filter, hook) in &self.watch_change_hooks {
      if filter.matches(&path_str, "file") {
        if let Some(r) =
          hook.on_watch_change(&WatchChangeArgs { path })
        {
          result.add_entries.extend(r.add_entries);
          result.remove_entries.extend(r.remove_entries);
        }
      }
    }

    result
  }

  /// Whether any watch change hooks are registered.
  pub fn has_watch_change_hooks(&self) -> bool {
    !self.watch_change_hooks.is_empty()
  }
}

/// Output of the transform pipeline for a single module.
pub struct TransformOutput {
  /// The final transformed content.
  pub content: String,
  /// The final loader after all transforms.
  pub loader: Loader,
  /// Composed source map (v3 JSON string).
  pub source_map: Option<String>,
}

// ---------------------------------------------------------------------------
// Built-in TypeScript transpiler as a transform hook
// ---------------------------------------------------------------------------

/// Built-in transform hook that transpiles TypeScript/TSX/JSX to JavaScript
/// using `deno_ast`.
pub struct BuiltinTranspiler;

impl OnTransform for BuiltinTranspiler {
  fn on_transform(&self, args: &TransformArgs) -> Option<TransformResult> {
    let media_type = match args.loader {
      Loader::Ts => deno_ast::MediaType::TypeScript,
      Loader::Tsx => deno_ast::MediaType::Tsx,
      Loader::Jsx => deno_ast::MediaType::Jsx,
      _ => return None, // Not a type that needs transpilation.
    };

    let specifier = if args.namespace == "file" {
      deno_ast::ModuleSpecifier::from_file_path(args.path).ok()?
    } else {
      deno_ast::ModuleSpecifier::parse(&format!(
        "file:///{}",
        args.path.display()
      ))
      .ok()?
    };

    let parsed = deno_ast::parse_module(deno_ast::ParseParams {
      specifier,
      text: args.content.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .ok()?;

    let emit_options = deno_ast::EmitOptions {
      source_map: deno_ast::SourceMapOption::Inline,
      inline_sources: true,
      ..Default::default()
    };

    let result = parsed
      .transpile(
        &deno_ast::TranspileOptions::default(),
        &deno_ast::TranspileModuleOptions::default(),
        &emit_options,
      )
      .ok()?;

    let text = match result {
      deno_ast::TranspileResult::Owned(e) => e.text,
      deno_ast::TranspileResult::Cloned(e) => e.text,
    };

    Some(TransformResult {
      content: Some(text),
      loader: Some(Loader::Js),
      source_map: None, // Inline in the content.
    })
  }
}

/// Create a `PluginDriver` with the built-in transpiler registered.
///
/// This is the default driver used when no external plugins are configured.
pub fn create_default_plugin_driver() -> PluginDriver {
  let mut build = PluginBuild::new();

  // Register the built-in transpiler at Normal order.
  // It matches any file with a TS/TSX/JSX loader.
  // The filter regex matches everything — the loader check is inside
  // the hook itself.
  build.on_transform(
    HookFilter::new(Regex::new(".*").unwrap()),
    Box::new(BuiltinTranspiler),
  );

  PluginDriver::from_build(build)
}

/// Create a `PluginDriver` with plugins plus the built-in transpiler.
pub fn create_plugin_driver(
  plugins: Vec<Box<dyn Plugin>>,
) -> PluginDriver {
  let mut build = PluginBuild::new();

  // Register plugin hooks first.
  for plugin in &plugins {
    plugin.setup(&mut build);
  }

  // Register the built-in transpiler at Normal order.
  build.on_transform(
    HookFilter::new(Regex::new(".*").unwrap()),
    Box::new(BuiltinTranspiler),
  );

  PluginDriver::from_build(build)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_hook_filter_matches() {
    let filter = HookFilter::new(Regex::new(r"\.ts$").unwrap());
    assert!(filter.matches("foo.ts", "file"));
    assert!(!filter.matches("foo.js", "file"));
    assert!(filter.matches("foo.ts", "virtual")); // No namespace constraint.
  }

  #[test]
  fn test_hook_filter_with_namespace() {
    let filter = HookFilter::with_namespace(
      Regex::new(r"\.ts$").unwrap(),
      "virtual".to_string(),
    );
    assert!(!filter.matches("foo.ts", "file")); // Wrong namespace.
    assert!(filter.matches("foo.ts", "virtual"));
  }

  #[test]
  fn test_transform_pipeline_basic() {
    let driver = create_default_plugin_driver();
    let path = Path::new("/project/src/mod.ts");
    let output = driver.transform(
      "const x: number = 42;".to_string(),
      path,
      "file",
      Loader::Ts,
    );

    // TS should be transpiled to JS.
    assert!(!output.content.contains(": number"));
    assert!(output.content.contains("42"));
    assert_eq!(output.loader, Loader::Js);
  }

  #[test]
  fn test_transform_pipeline_skips_js() {
    let driver = create_default_plugin_driver();
    let path = Path::new("/project/src/mod.js");
    let source = "const x = 42;";
    let output = driver.transform(
      source.to_string(),
      path,
      "file",
      Loader::Js,
    );

    // JS should pass through unchanged.
    assert_eq!(output.content, source);
    assert_eq!(output.loader, Loader::Js);
  }

  #[test]
  fn test_transform_pipeline_loader_change_reruns() {
    // A plugin that transforms .vue files to TSX.
    struct VuePlugin;
    impl OnTransform for VuePlugin {
      fn on_transform(
        &self,
        args: &TransformArgs,
      ) -> Option<TransformResult> {
        if args.path.extension()?.to_str()? == "vue" {
          Some(TransformResult {
            content: Some(
              "export default function App() { return <div>hello</div>; }"
                .to_string(),
            ),
            loader: Some(Loader::Tsx), // Change loader to TSX.
            source_map: None,
          })
        } else {
          None
        }
      }
    }

    let mut build = PluginBuild::new();

    // Vue plugin at Pre order (runs before built-in transpiler).
    build.on_transform_with_order(
      HookFilter::new(Regex::new(r"\.vue$").unwrap()),
      PluginOrder::Pre,
      Box::new(VuePlugin),
    );

    // Built-in transpiler at Normal order.
    build.on_transform(
      HookFilter::new(Regex::new(".*").unwrap()),
      Box::new(BuiltinTranspiler),
    );

    let driver = PluginDriver::from_build(build);

    let path = Path::new("/project/src/App.vue");
    let output = driver.transform(
      "<template>...</template>".to_string(),
      path,
      "file",
      Loader::Text, // Vue files load as text, plugin transforms to TSX.
    );

    // Should have been: .vue → TSX (by Vue plugin) → JS (by transpiler).
    assert_eq!(output.loader, Loader::Js);
    assert!(!output.content.contains("<div>")); // JSX should be transformed.
  }

  #[test]
  fn test_plugin_driver_empty() {
    let driver = PluginDriver::empty();
    let path = Path::new("/mod.js");
    let output = driver.transform(
      "const x = 42;".to_string(),
      path,
      "file",
      Loader::Js,
    );
    // No hooks registered, content passes through.
    assert_eq!(output.content, "const x = 42;");
  }

  #[test]
  fn test_resolve_first_wins() {
    struct Resolver1;
    impl OnResolve for Resolver1 {
      fn on_resolve(&self, _args: &ResolveArgs) -> Option<ResolveResult> {
        Some(ResolveResult::new(PathBuf::from("/resolved1")))
      }
    }

    struct Resolver2;
    impl OnResolve for Resolver2 {
      fn on_resolve(&self, _args: &ResolveArgs) -> Option<ResolveResult> {
        Some(ResolveResult::new(PathBuf::from("/resolved2")))
      }
    }

    let mut build = PluginBuild::new();
    build.on_resolve(
      HookFilter::new(Regex::new(".*").unwrap()),
      Box::new(Resolver1),
    );
    build.on_resolve(
      HookFilter::new(Regex::new(".*").unwrap()),
      Box::new(Resolver2),
    );

    let driver = PluginDriver::from_build(build);

    let args = ResolveArgs {
      specifier: "foo",
      importer: Path::new("/entry.js"),
      namespace: "file",
      kind: ResolveKind::Import,
    };

    let result = driver.resolve(&args).unwrap();
    assert_eq!(result.path, PathBuf::from("/resolved1")); // First wins.
  }
}
