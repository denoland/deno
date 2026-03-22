# Prototype: `deno check` using stock `tsc`

Replaces Deno's forked TypeScript with the **stock `tsc` binary** from
`npm:typescript`. No custom CompilerHost — only `tsconfig.json`, `paths`
mappings, and `@types` packages.

## Quick start

```bash
# From the deno2 repo root:
./target/debug/deno run --allow-all \
  prototype_tsc_check/deno_check.ts \
  prototype_tsc_check/demo/server.ts
```

The demo app (`demo/server.ts`) uses all supported import types:
- `npm:hono@4` and `npm:zod@3` — npm packages
- `jsr:@std/fmt@1/printf`, `jsr:@std/path@1/join`, `jsr:@std/assert@1` — JSR
- `import greeting from "./greeting.txt" with { type: "text" }` — text import
- `import favicon from "./favicon.png" with { type: "bytes" }` — bytes import
- `Deno.serve()`, `Deno.cwd()`, `Deno.env` — Deno APIs

## How it works

```
                 deno info --json
                       │
          ┌────────────┼────────────┐
          ▼            ▼            ▼
    jsr: specifiers  npm: specs  assertion imports
          │            │            │
          ▼            ▼            ▼
    vendor/ symlinks  paths →     .d.png.ts
    preserving URL    npm cache   .d.txt.ts
    path structure    types entry  sidecar files
          │            │            │
          └────────────┼────────────┘
                       ▼
              tsconfig.json with:
              • paths: { "jsr:..." → vendor/..., "npm:..." → cache/... }
              • types: ["deno", "node"]
              • allowArbitraryExtensions: true
              • module: "preserve"
                       │
                       ▼
              node_modules/
              ├── @types/deno/  (auto-installed)
              ├── @types/node/  (auto-installed)
              ├── hono → symlink to deno npm cache
              └── zod  → symlink to deno npm cache
                       │
                       ▼
                tsc --noEmit --project tsconfig.json
```

### Resolution strategies

| Specifier | How it works |
|-----------|-------------|
| `jsr:@std/fmt@1/printf` | `deno info` resolves → cached `.ts` file. Symlinked into `vendor/jsr.io/@std/fmt/1.0.9/printf.ts`. tsconfig `paths` maps the specifier to the symlink. Relative imports between JSR modules work because we preserve the URL directory structure. |
| `npm:zod@3` | `deno info` resolves → npm cache dir. tsconfig `paths` maps directly to the `types` entry in `package.json` (e.g. `index.d.ts`). |
| `https://example.com/mod.ts` | Same as JSR — symlinked into `vendor/example.com/mod.ts`, mapped via `paths`. |
| `import x from "./f.png" with { type: "bytes" }` | Generates sidecar `f.d.png.ts` declaring `Uint8Array`. tsc's `allowArbitraryExtensions` picks it up. Cleaned up after check. |
| `import x from "./f.txt" with { type: "text" }` | Same, sidecar declares `string`. |
| `Deno.*` APIs | `@types/deno` auto-installed into `node_modules/@types/deno/`. |
| `node:*`, Node globals | `@types/node` auto-installed into `node_modules/@types/node/`. |

### Key insights

1. **tsconfig `paths` supports colons** — `"jsr:@std/fmt@1/printf"` works as a
   paths key. Target just needs a proper `.ts` extension.

2. **URL structure = free transitive resolution** — vendoring
   `https://jsr.io/@std/fmt/1.0.9/printf.ts` as
   `vendor/jsr.io/@std/fmt/1.0.9/printf.ts` means when `printf.ts` does
   `import "./colors.ts"`, tsc finds `vendor/jsr.io/@std/fmt/1.0.9/colors.ts`.
   No extra path mappings needed for relative imports.

3. **`deno info --json` gives us everything** — the full graph including all
   transitive `jsr:` redirects, npm package metadata, and assertion import
   types. One subprocess call replaces the entire custom module resolution layer.

4. **`@types/deno` + `@types/node` replace custom globals** — no need for a
   hand-maintained Deno type stub.

## Generated artifacts

Everything goes into `.deno_check/` (can be gitignored):
- `.deno_check/vendor/` — symlinks to cached dependency sources
- `.deno_check/tsconfig.json` — generated tsconfig
- `node_modules/` — symlinks to deno's npm cache + @types packages

Sidecar `.d.{ext}.ts` files are created next to assets during check and cleaned
up after.
