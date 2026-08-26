# Agent & contributor documentation

This directory holds long-form, mostly-stable context about how the Deno
codebase is structured. It is meant to be read by both humans and AI coding
agents before they start working in the repository.

`CLAUDE.md` at the repository root is the short, operational guide: how to
build, which command runs which test, formatting and lint rules, the git
workflow. The files in `doc/` are the longer companion to it: the architectural
"why" and the map of where things live, the kind of background that is too large
to keep in `CLAUDE.md` but that saves a lot of exploration time up front.

## Contents

- [`architecture.md`](./architecture.md) — the layered design of the runtime:
  the `deno` CLI crate, the `deno_runtime` crate, the `ext/*` extensions, and
  the `deno_core` / `libs/*` foundation. Read this first.
- [`codebase-map.md`](./codebase-map.md) — a directory-by-directory map plus the
  handful of files worth understanding before anything else.
- [`testing.md`](./testing.md) — the test taxonomy (spec, unit, unit_node, node
  compat, WPT) and which command runs each one.
- [`ci.md`](./ci.md) — how the CI workflow is generated and how it decides which
  jobs to run, including the docs-only fast path that brought this directory
  into existence.
- [`desktop-architecture.md`](./desktop-architecture.md) — how `deno desktop`
  wires a native window to a Deno runtime: the load model, the ABI handshake,
  the two transports, and the lifecycle edges.
- [`package-management.md`](./package-management.md) — how `deno add` /
  `deno install <pkg>` add dependencies: the two flag parsers, the config
  writer, the shared install path, and the `--dev` / `--save-optional` /
  `--no-save` flags (including why `optionalDependencies` need special
  handling).

## Editing these docs

These files are plain Markdown and are formatted by `deno fmt` like the rest of
the repository. Run `tools/format.js` before committing. Keep prose hard-wrapped
at 80 columns to match the existing style.

A pull request that touches **only** `doc/` runs the `lint` job and nothing
else. The build, test, bench and `deno_core` jobs are skipped, because editing
Markdown cannot break the binary. See [`ci.md`](./ci.md) for how that is wired
up. If your PR changes anything outside `doc/`, the full pipeline runs as usual.
