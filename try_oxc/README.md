# OXC-powered `deno lint` and `deno fmt` (PoC)

This directory demonstrates the proof-of-concept integration that replaces
Deno's built-in linter (`deno_lint`) and formatter (`dprint`) with
[oxlint](https://oxc.rs/docs/guide/usage/linter.html) and
[oxfmt](https://oxc.rs/docs/guide/usage/formatter.html) from the
[OXC project](https://oxc.rs/).

## Quick Start

Build the patched Deno binary from the repo root:

```sh
cargo build --bin deno
```

Then try it on this demo project:

```sh
# Format all files (TS, CSS, JSON)
./target/debug/deno fmt try_oxc/

# Check formatting without writing
./target/debug/deno fmt --check try_oxc/

# Lint all files
./target/debug/deno lint try_oxc/

# Lint with JSON output
./target/debug/deno lint --json try_oxc/

# Lint with compact output
./target/debug/deno lint --compact try_oxc/

# Format via stdin
echo 'const x = {a:1,b:2}' | ./target/debug/deno fmt --ext=ts -
```

## What's in this demo

| File | Purpose |
|------|---------|
| `App.tsx` | React component with intentional lint issues |
| `utils.ts` | Utility functions with messy formatting |
| `styles.css` | CSS with minified formatting |
| `data.json` | JSON on a single line |
| `oxlintrc.json` | oxlint configuration |
| `.oxfmtrc.json` | oxfmt configuration |

### Intentional lint issues in `App.tsx`

- `debugger` statement (line 18) — caught by `no-debugger`
- `confirm()` call (line 19) — caught by `no-alert`
- `var` usage (line 47) — caught by `no-var`
- Unused variable (line 47) — caught by `no-unused-vars`

## Native Config Files

Both tools use their own native config files, which are auto-detected by
walking up from the linted/formatted files' directory.

### oxlint: `oxlintrc.json`

oxlint uses an ESLint v8-compatible configuration format. Place an
`oxlintrc.json` in your project root:

```json
{
  "$schema": "https://raw.githubusercontent.com/oxc-project/oxc/main/npm/oxlint/configuration_schema.json",
  "rules": {
    "no-debugger": "error",
    "no-var": "error",
    "no-unused-vars": "warn",
    "eqeqeq": "warn",
    "no-alert": "warn"
  }
}
```

Rules accept `"off"`, `"warn"`, or `"error"` severity levels, just like
ESLint. You can also use the array form for rules with options:

```json
{
  "rules": {
    "no-unused-vars": ["warn", { "argsIgnorePattern": "^_" }]
  }
}
```

Additional oxlint features available via config:

- **Plugins**: `--react-plugin`, `--jsdoc-plugin`, `--jsx-a11y-plugin` etc.
  can be enabled to get framework-specific rules
- **TypeScript-aware rules**: Use `--tsconfig=./tsconfig.json` for
  import resolution and type-aware linting
- **Nested configs**: oxlint can load configs from subdirectories
  (disable with `--disable-nested-config`)

See the full config schema:
https://oxc.rs/docs/guide/usage/linter/config.html

### oxfmt: `.oxfmtrc.json`

oxfmt uses a Prettier-compatible configuration format. Place an
`.oxfmtrc.json` in your project root:

```json
{
  "$schema": "https://oxc.rs/oxfmt/configuration_schema.json",
  "printWidth": 80,
  "tabWidth": 2,
  "useTabs": false,
  "semi": true,
  "singleQuote": false,
  "trailingComma": "all",
  "bracketSpacing": true,
  "jsxSingleQuote": false,
  "arrowParens": "always"
}
```

All Prettier options are supported. oxfmt also includes built-in support for:

- **Import sorting** — automatically organizes imports
- **Tailwind CSS class sorting** — sorts utility classes
- **package.json sorting** — orders fields conventionally
- **Embedded formatting** — CSS-in-JS, GraphQL in tagged templates

See the full config schema:
https://oxc.rs/docs/guide/usage/formatter.html

## How It Works

### Linting (`deno lint`)

1. On first run, the oxlint binary is auto-downloaded from npm
   (`@oxlint/<platform>`) and cached in `$DENO_DIR/dl/`
2. All files are passed to `oxlint --format json` in a single batch call
3. JSON diagnostics are parsed and mapped to Deno's `LintDiagnostic` type
4. Existing reporters (pretty, compact, JSON) render the output

### Formatting (`deno fmt`)

1. oxfmt is invoked via `deno run npm:oxfmt@<version>` (it ships as a
   NAPI module, not a standalone binary)
2. For `--check` mode, oxfmt's `--check` flag is used
3. For write mode, oxfmt's `--write` flag formats files in place
4. For stdin, content is piped through oxfmt with `--stdin-filepath`

### Config auto-detection

Both tools auto-detect their config files by walking up from the
directory of the files being processed. This means you can place config
files in subdirectories for per-project settings, or at the repo root
for workspace-wide defaults.

## Supported Languages

### oxfmt formats

TypeScript, JavaScript, JSX, TSX, JSON, JSONC, JSON5, CSS, SCSS, Less,
HTML, Markdown, MDX, YAML, TOML, Vue, Svelte, Astro, Angular, GraphQL

### oxlint lints

TypeScript, JavaScript, JSX, TSX — with 400+ rules covering ESLint
core, TypeScript-ESLint, React, JSX-A11y, Import, Jest, Vitest,
Unicorn, and more.

## Limitations (PoC)

- No `--fix` support for linting
- No incremental caching (every run processes all files)
- Deno-specific rules (`no-sloppy-imports`) still go through `deno_lint`
  as package-level rules
- `deno.json` lint/fmt config fields are not mapped to oxlint/oxfmt
  configs — use native config files instead
- stdin linting still uses `deno_lint` (not oxlint)
