# Vite-like Bundler with JavaScript Plugins for Deno

## Overview

Add a new bundler to Deno that serves as a **universal pre-processor/virtual
file system** for all Deno tooling:

- Uses **deno_ast** (not esbuild) for parsing, transformation, and code
  generation
- Supports **JavaScript-based plugins** following the linter plugin pattern
- Handles **multiple environments** (server/Deno, browser) simultaneously
- Implements a **two-layer module graph** (source → bundled chunks)
- Integrates as virtual file system for **all Deno tools**:
- `deno run` - run transformed code
- `deno test` - test transformed code
- `deno lint` - lint transformed code
- `deno check` - type check transformed code
- `deno compile` - compile transformed code

**Key Use Case:** Enable support for non-JS files like `.svelte`, `.vue`,
`.astro` that need transformation before they can be run, tested, linted, or
type-checked.

**Core Insight:** The bundler is really a **universal transformation layer**
that sits between the file system and all Deno tools. Bundling (combining into
chunks) is just one operation this layer can perform. The VFS can also:

- Transform individual files on-demand (lazy mode for
  `run`/`test`/`lint`/`check`)
- Bundle files into chunks (eager mode for `bundle`/`compile`)
- Provide type information to TypeScript (via generated `.d.ts`)

## Architecture

```
┌─────────────────────────────────────┐
│         Plugin Host                  │
│    (isolated V8 thread)              │
│  - .svelte → JS transformation       │
│  - .vue → JS transformation          │
│  - Custom file type handling         │
└───────────────┬─────────────────────┘
│
┌───────────────▼─────────────────────┐
│     Virtual File System (VFS)        │
│  - Maps original → transformed       │
│  - Source maps for error mapping     │
│  - Caches transformed results        │
└───────────────┬─────────────────────┘
│
┌───────────┬───────────┬───┴───┬───────────┬─────────────┐
│           │           │       │           │             │
▼           ▼           ▼       ▼           ▼             ▼
┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐
│deno run│  │deno    │  │deno    │  │deno    │  │deno    │  │deno    │
│        │  │test    │  │lint    │  │check   │  │compile │  │bundle  │
└────────┘  └────────┘  └────────┘  └────────┘  └────────┘  └────────┘
```

### Bundler-Specific Architecture (for `deno bundle`)

```
Bundler Core
├── Layer 1: Source Module Graph (multi-environment aware)
│   └── Cross-environment references tracked
│
└── Layer 2: Chunk Graphs (per-environment)
└── Only these are visible to runtime module loaders
```

## Key Components

### 1. Plugin System (`cli/tools/vbundle/plugins.rs`)

**Model after:** `cli/tools/lint/plugins.rs`

- Separate OS thread with isolated V8/Deno runtime
- Binary AST serialization (reuse `cli/tools/lint/ast_buffer/`)
- Message passing via channels for plugin requests

**Plugin API (JavaScript):**

```typescript
export default {
name: "plugin-name",
setup(build: BuildContext): void,
resolveId(source, importer, options): ResolveResult | null,
load(id, options): LoadResult | null,
transform: {
"ImportDeclaration": (node, context) => TransformResult,
"CallExpression[callee.name='require']": (node, context) => TransformResult,
},
renderChunk(code, chunk, options): string | null,
generateBundle(options, bundle): void,
}
```

**Ops to implement:**

- `op_bundler_resolve` - Plugin resolution results
- `op_bundler_load` - Plugin load results
- `op_bundler_emit_file` - Emit additional files
- `op_bundler_get_source` - Access source code

### 2. Multi-Environment Module Graph (`cli/tools/vbundle/source_graph.rs`)

**Layer 1 - Source Graph:**

```rust
pub struct SourceModuleGraph {
modules: HashMap<ModuleSpecifier, SourceModule>,
entrypoints: HashMap<BundleEnvironment, Vec<ModuleSpecifier>>,
cross_env_refs: HashMap<(ModuleSpecifier, BundleEnvironment), CrossEnvRef>,
}

pub enum BundleEnvironment {
Server,   // Deno runtime (default)
Browser,  // Browser runtime
Custom(String),
}
```

Cross-environment references allow server code to reference browser entrypoints
(for SSR + hydration patterns).

**Layer 2 - Chunk Graphs (`cli/tools/vbundle/chunk_graph.rs`):**

```rust
pub struct ChunkGraph {
environment: BundleEnvironment,
chunks: HashMap<ChunkId, Chunk>,
module_to_chunk: HashMap<ModuleSpecifier, ChunkId>,
}

pub struct Chunk {
id: ChunkId,
is_entry: bool,
modules: Vec<ModuleSpecifier>,
imports: Vec<ChunkId>,       // Static chunk deps
dynamic_imports: Vec<ChunkId>, // Dynamic chunk deps
code: Option<String>,
source_map: Option<String>,
}
```

### 3. Code Emission (`cli/tools/vbundle/emitter.rs`)

Use deno_ast for all transformations:

- `deno_ast::parse_program()` for parsing
- SWC visitor pattern (`VisitMut`) for transformations
- `deno_ast::emit()` for code generation
- Scope hoisting for chunk concatenation

**Reference:** `libs/resolver/emit.rs` for existing transpile patterns

### 4. Runtime Integration (`cli/tools/vbundle/bundle_module_loader.rs`)

Custom `ModuleLoader` that serves bundled chunks:

```rust
impl ModuleLoader for BundledModuleLoader {
fn resolve(&self, specifier, referrer, kind) {
// Map original specifier → chunk specifier
self.chunk_graph.module_to_chunk.get(&specifier)
}

fn load(&self, specifier, ...) {
// Return chunk code for chunk specifiers
self.chunks.get(specifier).code
}
}
```

### 5. Virtual File System (`cli/tools/vbundle/virtual_fs.rs`)

The VFS is the core abstraction enabling integration with all Deno tools:

```rust
pub struct BundlerVirtualFS {
/// Plugin host for on-demand transformation
plugin_host: Arc<PluginHost>,

/// Cache of transformed files: original specifier → transformed code
transformed_cache: DashMap<ModuleSpecifier, TransformedModule>,

/// Source maps: transformed position → original position
source_maps: DashMap<ModuleSpecifier, SourceMap>,

/// File extension handlers registered by plugins
extension_handlers: HashMap<String, PluginId>,
}

pub struct TransformedModule {
pub original_specifier: ModuleSpecifier,
pub code: Arc<str>,
pub source_map: Option<SourceMap>,
pub media_type: MediaType,  // Always JS/TS after transformation
}

impl BundlerVirtualFS {
/// Transform a file on-demand (lazy transformation)
pub async fn load(&self, specifier: &ModuleSpecifier) -> Result<TransformedModule> {
// Check cache first
if let Some(cached) = self.transformed_cache.get(specifier) {
return Ok(cached.clone());
}

// Determine if this file type needs transformation
let ext = specifier.extension();
if let Some(plugin_id) = self.extension_handlers.get(ext) {
// Call plugin to transform
let result = self.plugin_host.transform(specifier, plugin_id).await?;
self.transformed_cache.insert(specifier.clone(), result.clone());
return Ok(result);
}

// Pass through unchanged for native JS/TS
self.load_passthrough(specifier).await
}

/// Map error positions from transformed → original
pub fn map_error_position(&self, specifier: &ModuleSpecifier, pos: Position) -> Position {
if let Some(source_map) = self.source_maps.get(specifier) {
source_map.lookup(pos).unwrap_or(pos)
} else {
pos
}
}
}
```

### 6. Test Runner Integration

Integration with test runner:

1. Before test execution, set up VFS with plugins
2. Inject VFS-backed `ModuleLoader` into test workers
3. Tests import `.svelte`/`.vue` files transparently
4. Errors are mapped back to original source positions

### 7. Linter Integration (`cli/tools/lint/`)

The linter needs to lint the **transformed** output of non-JS files:

```rust
// cli/tools/lint/mod.rs modifications

pub async fn lint_with_vfs(
vfs: &BundlerVirtualFS,
specifiers: &[ModuleSpecifier],
) -> Result<Vec<LintDiagnostic>> {
let mut diagnostics = Vec::new();

for specifier in specifiers {
// Get transformed code from VFS
let transformed = vfs.load(specifier).await?;

// Lint the transformed JS/TS
let file_diagnostics = lint_file(&transformed.code, &transformed.media_type)?;

// Map diagnostic positions back to original source
for mut diag in file_diagnostics {
diag.range = vfs.map_error_position(specifier, diag.range);
diag.filename = specifier.to_string();  // Show original filename
diagnostics.push(diag);
}
}

Ok(diagnostics)
}
```

**Key considerations:**

- Lint the transformed output (valid JS/TS)
- Map error positions back to original `.svelte`/`.vue` source
- Show original filenames in diagnostics
- Plugins can emit lint-specific metadata (e.g., component props for unused var
  detection)

### 8. Type Checker Integration (`cli/tsc/`)

TypeScript needs to see transformed files as valid TS/JS:

```rust
// Integration with TypeScript language service

pub struct VfsTypeChecker {
vfs: Arc<BundlerVirtualFS>,
ts_server: TsServer,
}

impl VfsTypeChecker {
/// Provide transformed source to TypeScript
fn get_script_text(&self, specifier: &ModuleSpecifier) -> Option<String> {
// TypeScript asks for file content
// Return transformed JS/TS instead of original .svelte
let transformed = self.vfs.load_sync(specifier).ok()?;
Some(transformed.code.to_string())
}

/// Map TypeScript errors back to original source
fn map_diagnostics(&self, diagnostics: Vec<TsDiagnostic>) -> Vec<TsDiagnostic> {
diagnostics.into_iter().map(|mut diag| {
if let Some(file) = &diag.file {
let specifier = ModuleSpecifier::parse(file).ok()?;
diag.start = self.vfs.map_error_position(&specifier, diag.start);
diag.end = self.vfs.map_error_position(&specifier, diag.end);
}
diag
}).collect()
}
}
```

**Plugin-provided type information:** Plugins can emit `.d.ts` declarations
alongside transformed code:

```typescript
// Svelte plugin example
export default {
  name: "svelte",

  load(id) {
    if (!id.endsWith(".svelte")) return null;

    const { js, css, ast } = svelte.compile(source);

    // Generate type declarations for component props
    const dts = generateSvelteTypes(ast);

    return {
      code: js.code,
      map: js.map,
      // Additional type info for TypeScript
      declarations: dts,
    };
  },
};
```

## Implementation Phases

### Phase 1: Foundation

- Create `cli/tools/vbundle/` directory structure
- Implement basic `SourceModuleGraph` (single environment first)
- Port AST serialization from linter
- Create plugin host skeleton with setup/resolve/load hooks
- Add CLI flags (`deno vbundle` or integrate into existing build)

### Phase 2: Virtual File System Core

- Implement `BundlerVirtualFS` with lazy transformation
- Source map handling and position mapping
- File extension registration for plugins
- Caching layer for transformed modules

### Phase 3: Plugin System

- Full plugin API (resolveId, load, transform, renderChunk)
- Transform hooks with CSS-like AST selectors
- Plugin ordering and hook priority
- Built-in plugins: alias, define, json

### Phase 4: Core Bundling

- Single-environment bundling with deno_ast
- Basic code splitting on dynamic imports
- Chunk emission with simple concatenation
- Source map generation through bundle

### Phase 5: Multi-Environment

- Implement environment configs
- Layer 1: cross-environment reference tracking
- Layer 2: per-environment chunk generation
- Environment-specific transforms (e.g., `import.meta.env`)

### Phase 6: Runtime Integration (`deno run`)

- VFS-backed `ModuleLoader` implementation
- Transparent transformation of non-JS imports
- Error position mapping to original sources
- Hot module replacement foundation

### Phase 7: Test Runner Integration (`deno test`)

- Inject VFS into test workers
- Test discovery for transformed file types
- Error mapping in test output
- Watch mode with incremental re-transformation

### Phase 8: Linter Integration (`deno lint`)

- Lint transformed output from VFS
- Map lint diagnostics back to original source
- Plugin metadata for smarter linting (e.g., unused props)
- Support linting original syntax via plugin-specific rules

### Phase 9: Type Checker Integration (`deno check`)

- TypeScript language service integration with VFS
- Provide transformed sources to TS compiler
- Map type errors back to original source
- Plugin-provided `.d.ts` generation support
- Module declaration generation for non-JS files

### Phase 10: Polish & Compile Integration

- `deno compile` integration with VFS
- Caching for incremental builds
- Parallel transformation
- Performance optimization
- Documentation and examples

## Critical Files to Modify/Reference

| File                        | Purpose                                    |
| --------------------------- | ------------------------------------------ |
| `cli/tools/lint/plugins.rs` | Reference for plugin host architecture     |
| `cli/js/40_lint.js`         | Reference for JS plugin API patterns       |
| `cli/module_loader.rs`      | Integration point for runtime (`deno run`) |
| `libs/resolver/emit.rs`     | deno_ast emit/transpile patterns           |
| `cli/graph_container.rs`    | ModuleGraphContainer patterns              |
| `cli/tools/test/mod.rs`     | Test runner integration points             |
| `cli/tools/lint/mod.rs`     | Linter integration points                  |
| `cli/tools/lint/linter.rs`  | Core linting logic to wrap with VFS        |
| `cli/tsc/mod.rs`            | TypeScript integration                     |
| `cli/tsc/diagnostics.rs`    | Error position mapping reference           |
| `cli/tools/check/mod.rs`    | Type checking command integration          |

## New Files to Create

```
cli/tools/vbundle/
├── mod.rs              # Entry point, CLI handling
├── plugins.rs          # Plugin host (JS runtime in separate thread)
├── source_graph.rs     # Layer 1: Source module graph
├── chunk_graph.rs      # Layer 2: Bundled chunks
├── splitter.rs         # Code splitting algorithm
├── emitter.rs          # deno_ast code generation
├── environment.rs      # Environment configs
├── virtual_fs.rs       # Core VFS implementation
├── vfs_module_loader.rs    # VFS-backed ModuleLoader for runtime
├── vfs_lint_adapter.rs     # VFS adapter for linter
├── vfs_tsc_adapter.rs      # VFS adapter for TypeScript
├── source_map.rs       # Source map handling and position mapping
└── types.rs            # Shared types

cli/js/
└── 40_vbundle.js       # JavaScript plugin runtime
```

## Verification Plan

1. **Unit tests:** Test each component in isolation
2. **Spec tests:** Create `tests/specs/vbundle/` with scenarios for:

- Basic bundling
- Multi-environment bundling
- Plugin hooks (resolve, load, transform)
- VFS transformations

3. **End-to-end `.svelte` example:**

```bash
# All should work with .svelte files via VFS + svelte plugin
deno run app.svelte           # Run transformed code
deno test app_test.ts         # Test importing .svelte
deno lint app.svelte          # Lint transformed output
deno check app.svelte         # Type check with generated types
deno compile app.svelte       # Compile to binary
deno bundle app.svelte        # Bundle for distribution
```

4. **Source map verification:**

- Errors point to original `.svelte` line numbers
- Stack traces show original source
- Debugger steps through original code

5. **Multi-environment test:** Bundle SSR app with server + browser targets
6. **Plugin test:** Sample plugins (svelte, vue, mdx) exercising all hooks
7. **Performance:** Benchmark transformation speed and caching effectiveness

## Estimated Effort

This is a significant undertaking with 10 phases:

| Phases | Focus                      | Complexity                              |
| ------ | -------------------------- | --------------------------------------- |
| 1-2    | Foundation + VFS Core      | Medium - core abstractions              |
| 3      | Plugin System              | High - mirrors linter plugin complexity |
| 4-5    | Bundling + Multi-env       | High - novel architecture               |
| 6-7    | Runtime + Test Integration | Medium - leverages existing loaders     |
| 8-9    | Linter + TypeScript        | Medium-High - careful position mapping  |
| 10     | Polish + Compile           | Medium - integration work               |

**Reference points:**

- Linter plugin system (`plugins.rs` + `40_lint.js` + `ast_buffer/`): ~3000
  lines
- Current esbuild bundler (`cli/tools/bundle/`): ~3000 lines
- VFS + multi-tool integration: estimated ~5000+ lines

**Key complexity areas:**

- Source map handling across all tools
- TypeScript integration (providing virtual sources to TS compiler)
- Multi-environment chunk graphs
- Plugin execution model (async, parallel, ordered)

**Incremental delivery possible:**

- Phase 1-3: VFS + plugins usable for `deno run` with transformed files
- Phase 4-5: Full bundler functionality
- Phase 6-9: Progressive tool integration (each tool can be done independently)

If you need specific details from before exiting plan mode (like exact code
snippets, error messages, or content you generated), read the full transcript
at:
/Users/marvinh/.claude/projects/-Users-marvinh-dev-denoland-deno/2d7833cc-15c3-421a-8f7d-7791d7d0ffa7.jsonl
