# Design: Built-in Version Management

Status: Draft

Authors: (add yourself)

Tracking issues: #25724, #25749, #25035, #18406, #18440, #5214, #24157

## Summary

This document proposes first-class, built-in management of multiple Deno
versions on a single system. The goal is to let users install, list, cache,
prune, and select Deno versions through Deno itself, and to let a project
declare the version (or version range) it expects so that running the project
with the wrong version produces a clear, actionable result instead of subtle
behavioral differences.

The work is split into four phases so that the early, low-risk pieces can ship
independently and deliver value before the more invasive enforcement layer is
built.

## Motivation

There is sustained, repeated demand for this across many years and many issues:

- #5214 (2020): "Multiple versions on a system" — users do not want to depend on
  external tools like `nvm`.
- #18406: builtin version management, `deno --version 1.30.3 run ...`, plus a
  `.dvmrc`-style project file. Closed, but the underlying need persists.
- #18440: `deno upgrade` should cache downloaded tarballs instead of
  re-downloading the same version repeatedly. Closed.
- #24157: installation-script and "install a specific version" friction for
  Deno 2.0; today users install latest and then downgrade.
- #25035: support semver ranges in `deno upgrade --version=` (e.g. `1.41`,
  `~1.41.0`) instead of forcing exact versions.
- #25724: rustup-style version manager with per-project version requirements in
  `deno.json` and automatic checking before task execution.
- #25749: nvm-style management — `ls-remote`, `list`, `prune`, wildcards,
  offline/cached metadata, RC/LTS discovery.

These cluster into two complementary themes:

1. **Management** (nvm-style): discover, install, cache, list, prune, and select
   among many versions. (#5214, #18440, #25035, #25749)
2. **Enforcement** (rustup-style): a project declares the version it needs and
   Deno honors it automatically. (#18406, #25724)

The two themes are independent enough to build separately, but share the same
underlying primitive: a local store of multiple installed Deno versions plus a
resolver that maps a request (exact, range, channel, or project requirement) to
a concrete installed version.

## Goals

- Install and keep multiple Deno versions side by side, managed by Deno.
- Resolve a version request from: an exact version, a semver range, a channel
  (`stable`, `rc`, `lts`, `canary`), or a project requirement.
- Work offline against cached release metadata, with an explicit refresh.
- Let a project pin its required version (range) in `deno.json`.
- Produce clear, actionable messages on version mismatch.
- Reuse Deno's existing `deno_semver` crate and the existing `deno upgrade`
  download/verify pipeline rather than inventing new machinery.

## Non-goals

- Managing Node.js, npm, or other runtimes (this is Deno-only).
- Replacing OS package managers for the *initial* install of Deno itself; the
  bootstrap installer remains as-is. (We can later offer a thin `denoup`
  bootstrap, but it is out of scope for the core feature.)
- Per-directory automatic shell hooks of the kind nvm injects into `cd`. We
  prefer explicit invocation and project-file resolution at process start.

## Background: what exists today

`deno upgrade` already does most of the download/verify/replace work
(`cli/tools/upgrade.rs`). Its flags today (`UpgradeFlags` in
`cli/args/flags.rs`):

```
dry_run, force, release_candidate, canary, no_delta,
version, output, version_or_hash_or_channel, checksum, pr, branch
```

So Deno can already select stable / `rc` / `canary` / a PR / a branch / an exact
version, download the tarball, verify a checksum, and replace the current
executable in place. What is missing is: (a) keeping more than one version
around, (b) range/wildcard resolution, (c) cached/offline metadata, and (d) any
notion of a project-declared required version.

This means Phase 1 is mostly additive to an existing, working pipeline.

## Design overview

Three new pieces, shared across phases:

1. **Version store** — a directory (under `$DENO_DIR`, e.g.
   `$DENO_DIR/versions/<version>/deno`) holding multiple installed executables,
   one per version, plus a small index.
2. **Release metadata cache** — a cached, refreshable list of available versions
   per channel (stable, rc, lts, canary) so resolution works offline. Populated
   from the same source `deno upgrade` already queries.
3. **Resolver** — maps a request to a concrete version:
   - exact (`2.1.4`)
   - range / wildcard (`~2.1`, `2.1.*`, `^2`) via `deno_semver`
   - channel (`stable`, `rc`, `lts`, `canary`)
   - project requirement (from `deno.json`)
   Resolution prefers an already-installed version that satisfies the request;
   otherwise it consults the metadata cache; only then does it hit the network.

## Phased plan

### Phase 1 — Extend `deno upgrade` (range resolution + cache)

Lowest risk, no new top-level commands, directly closes #25035 and #18440.

- Accept semver ranges and wildcards in `deno upgrade <range>` and
  `--version=<range>`: `1.41`, `~1.41.0`, `2.1.*`, `^2`. Resolve to the highest
  release satisfying the range within the appropriate channel.
- Cache downloaded tarballs (and the resolved metadata) so repeatedly switching
  between two versions does not re-download (#18440).
- No behavioral change to the default `deno upgrade` (latest stable).

Deliverables: range parsing in the upgrade path, a tarball/metadata cache, tests
covering range resolution and cache hits.

### Phase 2 — Multi-version store + management subcommands

Introduces side-by-side versions and the nvm-style surface (#5214, #25749).

Proposed subcommands (final naming TBD — see Open questions):

- `deno upgrade --list` / `deno versions` — list locally installed versions and
  mark the active one.
- `deno upgrade --list-remote` (a.k.a. `ls-remote`) — list available versions
  per channel from cached metadata, with `--refresh` to update the cache.
- `deno upgrade --prune` — remove unused/old versions, keeping the active one
  and (configurably) the latest of each channel.
- Installing a version adds it to the store instead of overwriting; selecting a
  version updates the active symlink/shim.

Deliverables: the version store layout + index, the metadata cache refresh,
list/list-remote/prune behaviors, and migration of the current single-binary
layout into the store.

### Phase 3 — Per-project version requirement in `deno.json`

Declarative pinning (#18406, #25724), enforcement is still advisory here.

- New optional field in `deno.json`, e.g.:

  ```jsonc
  {
    "version": "~2.1" // semver range the project requires
  }
  ```

- On `deno run` / `deno task` / etc., if the running Deno does not satisfy the
  project's `version` range, emit a clear warning (not yet auto-switching):

  ```
  warning: this project requires Deno ~2.1, but you are running 2.0.6.
    Run `deno upgrade ~2.1` to install a compatible version.
  ```

- Resolution reuses the Phase 1 resolver. No automatic switching yet, so there
  is no surprising behavior and no new trust surface.

Deliverables: schema addition + validation, the pre-run check and message, docs.
Field name and whether it lives at top level or under a namespace is an open
question.

### Phase 4 — Automatic selection (wrapper / shim)

The most invasive piece (#25724's wrapper proposal); strictly opt-in.

- A thin wrapper/shim on `PATH` reads the project's required `version`, resolves
  it against the store, and execs the matching installed executable — installing
  it first if missing and permitted.
- Must be explicitly enabled (config and/or env), default off, so users who do
  not want magic dispatch never pay for it.
- Needs careful handling of: the bootstrap problem (who installs the first
  version), `--version`/`upgrade` running against the *wrapper* vs the *real*
  binary, shell completions, and CI environments where auto-install is
  undesirable.

Deliverables: wrapper executable, opt-in config, auto-install policy, and an
escape hatch (e.g. `DENO_VERSION_PIN=off` / a flag) to bypass dispatch.

## Affected code (initial pointers)

- `cli/tools/upgrade.rs` — download/verify/replace pipeline; extend for ranges,
  caching, and the version store.
- `cli/args/flags.rs` — `UpgradeFlags` and new subcommand/flag surface.
- `cli/args/deno_json` (config schema) — the `version` field (Phase 3).
- `deno_semver` — range parsing/matching (reused, not reinvented).
- `$DENO_DIR` layout — new `versions/` store and metadata cache.

## Open questions

1. **Command surface**: flags on `deno upgrade` (`--list`, `--prune`,
   `--list-remote`) vs. a dedicated noun (`deno versions ...`). The latter is
   cleaner but adds a top-level command.
2. **`deno.json` field**: name (`version`? `deno`? `engines.deno` à la npm?),
   location, and whether ranges or exact-only.
3. **Store location & format**: `$DENO_DIR/versions/` layout, index format, and
   how the "active" version is represented (symlink vs. recorded pointer).
4. **Auto-install policy** in Phase 4: when is it acceptable to fetch a missing
   version automatically (interactive vs. CI), and how to make that obvious.
5. **Channel semantics** for ranges: should `~2.1` ever resolve to an RC, or
   only stable unless an `rc`/`canary` channel is explicitly requested?
6. **Interaction with OS package managers / existing installs**: how a
   Homebrew/apt-installed Deno coexists with a Deno-managed store.

## Rollout

- Ship Phase 1 behind no flag (pure addition to `deno upgrade`).
- Ship Phase 2 as the store + management commands; keep single-binary installs
  working by treating them as a one-entry store.
- Ship Phase 3 advisory-only; gather feedback on the `version` field before
  building Phase 4.
- Ship Phase 4 opt-in only.

## Prior art

- rustup: channels, `rust-toolchain.toml` per-project pinning, transparent
  wrapper dispatch. Closest analog to the full vision.
- nvm / fnm / volta: multi-version install + selection; `.nvmrc`. Closest analog
  to Phase 2/3 management ergonomics.
- The existing `deno upgrade` channels (`rc`, `lts`, `canary`) already establish
  the channel vocabulary we extend here.
