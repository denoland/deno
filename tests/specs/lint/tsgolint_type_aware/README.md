# tsgolint type-aware lint — manual verification

This fixture exercises type-aware linting via `tsgolint`. It is **not** a CI
spec test because the spec-test npm registry is a local mock that does not serve
the `@oxlint-tsgolint/*` packages the binary is downloaded from. Protocol
decoding and rule resolution are covered by unit tests in
`cli/tools/lint/tsgolint.rs`.

## Run it

Type-aware linting runs as part of `deno lint` by default. The `tsgolint` binary
is downloaded from npm into `DENO_DIR` automatically on first use (the same way
`deno bundle` fetches esbuild), so no manual setup is needed:

```sh
../../../../target/debug/deno lint floating.ts
```

To point at a locally built or hand-downloaded binary instead of the
auto-downloaded one, set `DENO_TSGOLINT_BIN=/path/to/tsgolint`. To turn
type-aware linting off, pass `--no-type`.

## Expected

`work();` in `floating.ts` is reported by the `no-floating-promises` type-aware
rule (exit code 1); the awaited call is not.

`ignore_test.ts` has the same floating call but with
`// deno-lint-ignore no-floating-promises` above it; linting it exits 0 with no
output. This is the key property: ignore directives apply to tsgolint
diagnostics because they flow back through deno_lint's external-linter path, so
deno_lint's ignore-directive filtering (and `ban-unused-ignore`) covers them.

`deno lint --no-type floating.ts` exits 0: the `--no-type` flag skips tsgolint
entirely, so the floating promise is not reported.

Both files are listed in `tsconfig.json` `include` so tsgolint type-checks them.
Even without a `tsconfig.json`, tsgolint would still lint them under its
built-in inferred project, so the type-aware rules run out of the box.
