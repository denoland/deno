# tsgolint type-aware lint — manual verification

This fixture exercises type-aware linting via `tsgolint`. It is **not** a CI
spec test because it requires the external `tsgolint` binary, which is not
available in CI yet (auto-download is a follow-up). Protocol decoding and rule
resolution are covered by unit tests in `cli/tools/lint/tsgolint.rs`.

## Run it

1. Obtain a `tsgolint` binary. The reliable way is the prebuilt platform binary
   shipped on npm (building from source needs tsgolint's codegen, not a plain
   `go build`):

   ```sh
   # picks the right @oxlint-tsgolint/<os>-<arch> binary for your platform
   npm pack @oxlint-tsgolint/darwin-arm64   # or linux-x64, etc.
   tar xzf oxlint-tsgolint-*.tgz
   # binary is at package/tsgolint
   ```

2. From this directory, run the dev build with the feature enabled:

   ```sh
   DENO_UNSTABLE_TSGOLINT=1 \
   DENO_TSGOLINT_BIN=/path/to/tsgolint \
     ../../../../target/debug/deno lint floating.ts
   ```

## Expected

`work();` in `floating.ts` is reported by the `no-floating-promises` type-aware
rule (exit code 1); the awaited call is not.

`ignore_test.ts` has the same floating call but with
`// deno-lint-ignore no-floating-promises` above it; linting it exits 0 with no
output. This is the key property: ignore directives apply to tsgolint
diagnostics because they flow back through deno_lint's external-linter path, so
deno_lint's ignore-directive filtering (and `ban-unused-ignore`) covers them.

Both files are listed in `tsconfig.json` `include` so tsgolint type-checks them.
