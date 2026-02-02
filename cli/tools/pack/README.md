# `deno pack` - Create npm-compatible packages from Deno projects

The `deno pack` command creates npm-compatible tarballs from Deno projects, transpiling TypeScript to JavaScript and generating proper npm package metadata.

## Features

### ✅ Core Functionality
- **TypeScript Transpilation**: Converts `.ts`/`.tsx` → `.js`, `.mts` → `.mjs`
- **Type Declarations**: Generates `.d.ts` files from fast_check (when available)
- **Specifier Rewriting**: Transforms Deno-style imports to npm-compatible ones
- **Dependency Detection**: Automatically extracts JSR and npm dependencies
- **npm Tarball Format**: Creates standard `.tgz` with `package/` prefix
- **package.json Generation**: Converts `deno.json` to npm package manifest

### 🔧 CLI Options

```bash
deno pack [OPTIONS]

Options:
  -o, --output <FILE>        Output file path (defaults to <name>-<version>.tgz)
      --dry-run              Show what would be packed without creating the tarball
      --allow-slow-types     Skip .d.ts generation (types will not be included)
      --allow-dirty          Allow packing if the repository has uncommitted changes
      --set-version <VERSION> Override the version in the tarball
      --no-deno-shim         Don't automatically add @deno/shim-deno dependency
      --config <FILE>        Specify deno.json config file
      --no-config            Don't use deno.json
```

### 📦 Specifier Rewriting

The command automatically rewrites import specifiers for npm compatibility:

| Deno Format | npm Output |
|-------------|------------|
| `./utils.ts` | `./utils.js` |
| `./utils.tsx` | `./utils.js` |
| `./utils.mts` | `./utils.mjs` |
| `jsr:@std/path` | `@std/path` |
| `jsr:@std/path@^1.0.0` | `@std/path` |
| `npm:express` | `express` |
| `npm:express@4.18.0` | `express` |
| `node:fs` | `node:fs` (unchanged) |

### 🔍 Automatic Dependency Detection

Dependencies are automatically extracted from import specifiers and added to `package.json`:

```json
{
  "dependencies": {
    "@std/path": "^1.0.0",
    "@deno/shim-deno": "~0.19.0"
  }
}
```

The `@deno/shim-deno` dependency is automatically added when Deno APIs are detected in the code (e.g., `Deno.version`, `Deno.readFile`).

## Usage Examples

### Basic Usage

```bash
# Create tarball from current directory
deno pack

# Output: @scope-package-1.0.0.tgz
```

### With Custom Output

```bash
deno pack --output my-package.tgz
```

### Dry Run

```bash
deno pack --dry-run
# Shows what would be included without creating the tarball
```

### Override Version

```bash
deno pack --set-version 2.0.0
# Creates @scope-package-2.0.0.tgz regardless of deno.json version
```

### Allow Uncommitted Changes

```bash
deno pack --allow-dirty
# Bypasses git clean check
```

## Required deno.json Fields

The following fields are required in `deno.json`:

```json
{
  "name": "@scope/package-name",  // Required: must be scoped
  "version": "1.0.0",             // Required: semver version
  "exports": "./mod.ts"           // Required: entry point(s)
}
```

### Multiple Exports

```json
{
  "name": "@scope/package",
  "version": "1.0.0",
  "exports": {
    ".": "./mod.ts",
    "./utils": "./utils.ts"
  }
}
```

Generates:

```json
{
  "exports": {
    ".": {
      "types": "./mod.d.ts",
      "import": "./mod.js",
      "default": "./mod.js"
    },
    "./utils": {
      "types": "./utils.d.ts",
      "import": "./utils.js",
      "default": "./utils.js"
    }
  }
}
```

## Output Structure

The generated tarball contains:

```
package/
  ├── package.json       # Generated npm manifest
  ├── mod.js            # Transpiled JavaScript
  ├── mod.d.ts          # Type declarations
  ├── utils.js          # Additional modules
  └── utils.d.ts
```

## Error Handling

### Missing Required Fields

```bash
error: Missing 'name' field in 'file:///path/to/deno.json'. Add a package name like:
  {
    "name": "@scope/package-name",
    ...
  }
```

### Invalid Package Name

```bash
error: Invalid package name 'my-package'. Package name must be in the format '@scope/name'
```

### Uncommitted Changes

```bash
error: Git repository has uncommitted changes. Use --allow-dirty to pack anyway.
?? new-file.ts
M  existing-file.ts
```

## Implementation Details

### Module Processing Pipeline

1. **Graph Building**: Creates module graph from exports using `create_publish_graph()`
2. **Fast Check**: Builds type graph with `fast_check_dts: true` for .d.ts generation
3. **File Collection**: Collects all modules from the graph
4. **Transpilation**: Transpiles TS → JS using `deno_ast`
5. **Type Extraction**: Extracts `.d.ts` from fast_check when available
6. **Specifier Rewriting**: Rewrites imports for npm compatibility
7. **Dependency Extraction**: Collects JSR/npm dependencies from imports
8. **package.json Generation**: Converts deno.json to npm manifest
9. **Tarball Creation**: Packages everything in npm-compatible format

### File Structure

```
cli/tools/pack/
  ├── mod.rs                  # Main pipeline orchestration
  ├── npm_tarball.rs          # Tarball creation with package/ prefix
  ├── package_json.rs         # deno.json → package.json conversion
  ├── specifier_rewriter.rs   # Import specifier transformation
  └── README.md              # This file
```

## Testing

Spec tests are located in `tests/specs/pack/`:

- `basic/` - Basic pack functionality
- `dry_run/` - Dry run mode
- `specifier_rewriting/` - Import rewriting
- `missing_name/` - Error handling
- `set_version/` - Version override
- `git_dirty/` - Git status checks

Run tests:

```bash
cargo test spec::pack
```

## Known Limitations

1. **Workspace Support**: Multi-package workspaces work but need additional testing
2. **Complex Specifiers**: Dynamic imports and template literals in import paths are not rewritten
3. **.d.ts Generation**: Falls back to stubs when fast_check .d.ts is unavailable

## Future Enhancements

- [ ] Support for custom TypeScript compiler options
- [ ] More sophisticated dependency version resolution
- [ ] Integration with npm publish workflow
- [ ] Support for non-scoped package names (if needed)
- [ ] Automatic README.md inclusion
- [ ] License file detection and inclusion
