# Replacing Deno's TypeScript Fork with Stock `tsc`

## Problem

Deno maintains a fork of TypeScript for `deno check` and the LSP. This fork is
hard to maintain, has subpar performance compared to upstream, and creates a
growing maintenance burden. The goal is to replace it with the stock
`npm:typescript` package, configured entirely through `tsconfig.json` — no
custom `CompilerHost`, no programmatic API.

## Solution

Use `deno info --json` to resolve the full module graph, then generate a
standard `tsconfig.json` + `node_modules/` layout that stock `tsc` understands.

### Architecture

```
User code (server.ts)
    │
    ▼
deno info --json          ← resolves all specifiers to local cached files
    │
    ├── jsr: specifiers   → node_modules/@jsr/scope__name/ (symlinks to cache)
    ├── npm: specifiers   → node_modules/name/ (symlinks to cache)
    ├── https: URLs       → node_modules/@jsr/ (if jsr.io) or paths mapping
    ├── bytes/text imports → node_modules/@deno/shims/ (wildcard declarations)
    ├── Deno.* APIs       → node_modules/@types/deno/ (auto-installed)
    └── Node APIs         → node_modules/@types/node/ (auto-installed)
    │
    ▼
tsconfig.json             ← paths mappings + standard compiler options
    │
    ▼
tsc --noEmit --project tsconfig.json
```

### Generated project layout

```
project/
├── server.ts              # user code (unchanged)
├── tsconfig.json          # generated, standard — editors pick it up
└── node_modules/          # standard node_modules, gitignored
    ├── @jsr/              # JSR packages
    │   ├── std__fmt/      #   @std/fmt → symlinks to deno's remote cache
    │   │   ├── printf.ts
    │   │   └── colors.ts
    │   ├── std__path/
    │   └── ...
    ├── @types/
    │   ├── deno/          # auto-installed via `deno cache npm:@types/deno`
    │   └── node/          # auto-installed via `deno cache npm:@types/node`
    ├── @deno/
    │   └── shims/         # wildcard module declarations for asset imports
    │       ├── index.d.ts
    │       └── package.json
    ├── hono/              # npm packages (symlinks to deno's npm cache)
    └── zod/
```

## How Each Specifier Type Is Handled

### `jsr:` specifiers

**Example:** `import { sprintf } from "jsr:@std/fmt@1/printf"`

1. `deno info --json` resolves `jsr:@std/fmt@1/printf` →
   `https://jsr.io/@std/fmt/1.0.9/printf.ts` → local cached file (hash-named,
   no extension)
2. We parse the jsr.io URL to extract scope (`std`), name (`fmt`), version
   (`1.0.9`), and file path (`printf.ts`)
3. Create `node_modules/@jsr/std__fmt/printf.ts` as a symlink to the cached file
   (the `@jsr/scope__name` convention matches what JSR's npm registry uses)
4. Add `tsconfig.json` paths entry:
   `"jsr:@std/fmt@1/printf" → ["node_modules/@jsr/std__fmt/printf.ts"]`

**Transitive deps work automatically:** when `printf.ts` does
`import "./colors.ts"`, tsc finds `node_modules/@jsr/std__fmt/colors.ts`
because we preserve the package's internal directory structure.

**Key tsconfig option:** `preserveSymlinks: true` — without this, tsc follows
symlinks to the hash-named cache file and resolves relative imports from there
(wrong directory). With it, relative imports resolve from the symlink's
location in `node_modules/@jsr/`.

### `npm:` specifiers

**Example:** `import { z } from "npm:zod@3"`

1. `deno info --json` resolves `npm:zod@3` → `npm:/zod@3.25.76` and provides
   the npm package metadata including name and version
2. The package already exists in deno's npm cache at
   `~/Library/Caches/deno/npm/registry.npmjs.org/zod/3.25.76/`
3. We read `package.json` to find the types entry point (`types`, `typings`,
   `exports["."].types`, or fallback to `index.d.ts`)
4. Add `tsconfig.json` paths entry:
   `"npm:zod@3" → ["/path/to/cache/zod/3.25.76/index.d.cts"]`
5. Create `node_modules/zod` symlink → cache dir (for transitive resolution)

### `https://` URL imports

**Example:** `import { red } from "https://jsr.io/@std/fmt/1.0.9/colors.ts"`

For jsr.io URLs: handled identically to JSR specifiers — the file is already
vendored into `node_modules/@jsr/`, just add a paths entry.

For non-jsr URLs: would need a separate vendor directory (not yet implemented
in prototype). Could use `node_modules/@url/hostname/path` or similar.

### `with { type: "bytes" }` and `with { type: "text" }` imports

**Example:** `import data from "./favicon.png" with { type: "bytes" }`

Uses **wildcard module declarations** in `node_modules/@deno/shims/index.d.ts`:

```typescript
declare module "*.png" { const data: Uint8Array; export default data; }
declare module "*.txt" { const data: string; export default data; }
// ... etc for common extensions
```

This covers the common case where binary formats are imported as bytes and text
formats as text. TypeScript doesn't validate the `with { type }` attribute
value — it just preserves it when `module: "preserve"` is set.

**Tradeoff:** the type is tied to the file extension, not the `with` attribute.
If someone imports a `.txt` as `bytes`, they'd get `string` instead of
`Uint8Array`. This is rare enough to be acceptable.

### Deno APIs (`Deno.*`)

Handled by `@types/deno` from npm (DefinitelyTyped). Auto-installed via
`deno cache npm:@types/deno` and symlinked into `node_modules/@types/deno/`.
Currently at ~13,600 lines covering the full Deno namespace.

### Node APIs (`node:*`, `Buffer`, etc.)

Handled by `@types/node` from npm. Auto-installed the same way.

## Key tsconfig.json Options

```jsonc
{
  "compilerOptions": {
    "target": "ESNext",           // Deno supports latest JS features
    "module": "preserve",         // Required for import attributes (with {})
    "moduleResolution": "bundler",// Supports paths, .ts imports, etc.
    "lib": ["ESNext", "DOM", "DOM.Iterable"],
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true,         // Skip errors in .d.ts files
    "esModuleInterop": true,
    "allowImportingTsExtensions": true,  // Deno allows .ts in imports
    "allowJs": true,
    "resolveJsonModule": true,
    "preserveSymlinks": true,     // Critical: resolve from symlink location
    "paths": {
      "jsr:@std/fmt@1/printf": ["node_modules/@jsr/std__fmt/printf.ts"],
      "npm:zod@3": ["/cache/path/zod/3.25.76/index.d.cts"],
      // ... one entry per non-standard specifier
    },
    "baseUrl": ".",
    "types": ["deno", "node", "@deno/shims"]
  }
}
```

### Why each option matters

- **`module: "preserve"`** — required for `with { type: "bytes" }` syntax.
  Without this, tsc rejects import attributes.
- **`moduleResolution: "bundler"`** — allows `paths` mappings for non-standard
  specifiers and `.ts` extension imports.
- **`preserveSymlinks: true`** — without this, tsc follows symlinks in
  `node_modules/@jsr/` back to the hash-named cache files and resolves relative
  imports from the wrong directory.
- **`allowImportingTsExtensions: true`** — Deno allows `import "./foo.ts"` but
  stock tsc rejects `.ts` extensions by default.
- **`skipLibCheck: true`** — avoids type errors in dependency `.d.ts` files
  (e.g., conflicts between `@types/node` and DOM lib).
- **`paths`** — the core mechanism. Maps every `jsr:` and `npm:` specifier to
  a local file. TypeScript `paths` supports colons and special characters in
  keys despite what you might expect.

## Key Discoveries

1. **tsconfig `paths` supports colons.** `"jsr:@std/fmt@1/printf"` works as a
   paths key. The initial assumption that it wouldn't work was wrong — the
   actual failure was that target files lacked `.ts` extensions.

2. **`preserveSymlinks` is critical.** Without it, symlinked JSR files resolve
   relative imports from the cache directory (hash-named files) instead of from
   `node_modules/@jsr/`. This one flag makes the entire node_modules approach
   work.

3. **`deno info --json` provides everything needed.** The full module graph
   including all transitive `jsr:` redirects, npm package metadata, assertion
   import types, and local cached file paths. One subprocess call replaces the
   entire custom module resolution layer.

4. **JSR's URL structure enables free transitive resolution.** By preserving
   the package's internal file structure in `node_modules/@jsr/scope__name/`,
   relative imports between files within a JSR package just work.

5. **Wildcard `declare module` eliminates sidecar files.** Instead of generating
   `favicon.d.png.ts` next to every asset, a single `index.d.ts` with
   `declare module "*.png" { ... }` covers all files of that extension.

6. **`@types/deno` exists on npm.** No need to maintain a custom Deno type stub
   — DefinitelyTyped already publishes `@types/deno` with ~13,600 lines of
   declarations.

## Editor / LSP Integration

Since `tsconfig.json` is at the project root and `node_modules/` has standard
layout, **stock `tsserver` (VS Code, etc.) works automatically:**

- Intellisense, completions, hover types for all import types
- Go-to-definition works through `paths` mappings into `node_modules/@jsr/`
- `@types/deno` provides Deno API completions
- `@deno/shims` provides types for asset imports
- No Deno-specific VS Code extension needed for type checking

The implication is that `deno check` generates the tsconfig + node_modules once,
and the editor picks it up for ongoing development. When new imports are added,
re-running `deno check` regenerates the mappings.

## What This Replaces

### Currently removed (handled by stock tsc)

- `cli/tsc/00_typescript.js` — 8.4MB bundled TypeScript compiler
- `cli/tsc/99_main_compiler.js` — custom compiler wrapper
- `cli/tsc/97_ts_host.js` — 932-line custom host with 12+ ops
- `cli/type_checker.rs` — 1145-line type checker orchestration
- `cli/tsc/mod.rs` — custom module resolution, op registration
- `cli/tsc/diagnostics.rs` — diagnostic filtering/rewriting
- All custom ops: `op_load`, `op_resolve`, `op_emit`, `op_is_node_file`,
  `op_remap_specifier`, `op_create_hash`, etc.

### Still used (not affected)

- **Transpilation:** `deno_ast` / SWC (never used the TS fork)
- **`deno doc`:** `deno_doc` crate (parses AST directly, no type checking)
- **`deno compile` / `deno bundle`:** transpilation only via `deno_ast`
- **Module graph building:** `deno_graph` crate (used by `deno info --json`)

### Intentional regressions (acceptable tradeoffs)

- **No dual global types.** `setTimeout` returns the same type everywhere
  (previously returned `number` in Deno code, `object` in npm packages). This
  is being removed anyway.
- **No custom diagnostic messages.** Users see stock tsc errors instead of
  Deno-specific suggestions like "add `/// <reference lib="deno.ns" />`". The
  stock errors are still accurate.
- **No fast-check caching.** Deno's custom type summary cache is lost. Stock
  tsc `--incremental` with `tsBuildInfoFile` can replace it.
- **Import maps not yet translated.** `deno.json` `imports` field needs to be
  converted to tsconfig `paths`. Straightforward to implement.
- **`deno.json` `compilerOptions` not forwarded.** User tsconfig options (jsx,
  strict, etc.) need to be merged into the generated tsconfig. Also
  straightforward — or users can just use `tsconfig.json` directly.

## Running the Prototype

```bash
# From the deno2 repo root:
./target/debug/deno run --allow-all \
  prototype_tsc_check/deno_check.ts \
  prototype_tsc_check/demo/server.ts
```

The demo app uses jsr (`@std/fmt`, `@std/path`, `@std/assert`), npm (`hono`,
`zod`), text imports, bytes imports, and Deno APIs — all type-checked by stock
TypeScript 5.9.

## Future Work

- **`deno.json` import maps → tsconfig `paths`**: translate the `imports` field
- **Non-jsr URL imports**: vendor into `node_modules/@url/` or similar
- **`tsc --incremental`**: wire up `tsBuildInfoFile` for caching
- **Watch mode**: re-run `deno info` + regenerate tsconfig on file changes
- **Publish `@deno/shims`**: could be a real npm package instead of generated
- **`@types/deno` version pinning**: match the @types version to the deno version
