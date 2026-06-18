# Deno 2.9 milestone — honest assessment

Working notes on the 2.9.0 milestone: what it is, whether the feature set
coheres, what to drop, and what to add. Snapshot of milestone data as of
2026-06-17 (GitHub milestone #73). Opinionated; meant to drive cuts and
additions, not to be neutral.

## Reality check

- 24 closed / 37 open (61 total, ~39% done), **no due date set**.
- Minor cadence is ~2 months (2.8.0 shipped 2026-05-22), so a realistic freeze
  is roughly late July / early August.
- Heavy single-author concentration (most PRs are @bartlomieju). With an agent
  fleet this is a throughput question, not a blocker, but the milestone is
  overcommitted and undated.

## Does the feature set make sense?

Yes — it resolves into four coherent pillars plus a permissions thread:

1. **Package management / npm parity** (strongest, most complete):
   add/remove/install/outdated/audit/why/publish/publish-to-npm/list/
   migrate-pnpm/seed-lockfile/git-deps/tarballs/`patch`. A real "you don't need
   npm/pnpm" story.
2. **`deno compile`**: persist storage/KV (#34618), payload breakdown (#34577),
   WASI run (#34719), bundle-default (#34802), include-as-is (merged).
3. **Test runner**: shard, thresholds, retry, each, snapshot (#35139), JSON
   (#35055) + JUnit reporters. Closing in on best-in-class.
4. **fmt**: editorconfig (#34071), overrides (#34068), sort imports (#33313),
   trailing comma (merged), lax-sql (merged).

Two things dilute the set:

- **#35194 zero-config JSX with an in-binary Preact runtime** is the odd one out
  — every other feature targets the pro TS/npm toolchain persona; this targets a
  zero-config beginner persona and embeds a specific framework. Different
  product bet; deserves its own RFC, not a quiet ride in a minor.
- **Web/networking items are scattered, not a pillar**: web locks (#31166),
  unix-socket (#32094), happy eyeballs (merged), userAgentData (merged),
  disable-compression (#13830). Either commit to a networking theme or accept as
  misc.

## What to drop / defer

- **oxlint subprocess replacing deno_lint (#32852)** — strategic engine swap,
  draft, broad behavior-change blast radius. 3.0 runway, not a 6-week minor.
- **Preact JSX (#35194)** — see above; defer pending RFC.
- **split --allow-net connect/listen (#34036)** — permission-model semantics,
  large, CONFLICTING. Pair with the per-package-permissions work already in 3.0
  (#34943) and do it deliberately there.
- **compile --bundle default (#34802)** — a default-behavior change; make it a
  deliberate decision with migration notes.
- **native HMR (#34944)** and **deno types (#34934)** — draft/prototype; let
  them land when ready rather than gating the release.
- **Stale external PRs cluttering the milestone**: #27271 (json outdated, opened
  Dec 2024), #31166 (web locks), #32094, #33104, #32820. Either someone owns
  finishing each this cycle or take them out of the milestone.

## What to add — theme-completers

Additions that finish a pillar strengthen the narrative; random ones dilute it.
Note: `deno audit` and `deno why` and a JUnit reporter already exist — verified
in-tree, do not re-add.

- **Test runner -> native mocking + fake timers.** The one obvious hole; mocking
  required `@std/testing/mock` until now. IN PROGRESS via the node:test work
  (mock.timers merged #33946; correctness fixes #35297; shared core W6). Tracked
  in `TESTING_DOC.md`. Decision (2026-06-18): build first-class mocking inside
  node:test, lazy-loaded / zero-cost when unused.
- **mock.module** (ESM/CJS) — node:test W4; gated on registerHooks
  (#35026/#35027/#35028).
- **Package mgmt -> `overrides`/`resolutions` in deno.json.** Verified missing
  (only `patch` exists). The missing other half of `deno audit`: audit finds the
  vulnerable transitive dep, overrides pins the fix. Highest-leverage add. Plus
  **pnpm catalog** support (rounds out migrate-pnpm) and `deno dedupe`.
- **`deno compile` -> code signing + macOS notarization** of the output binary.
  Not present today (signing machinery exists only in `deno desktop`). The real
  wall users hit distributing compiled binaries; turns compile into a shipping
  pipeline.
- **Deliberate web-standards call**: either a real pillar (Temporal is not
  native yet — headline-grade) or consolidate the scattered net items under one
  banner.

## Bottom line

Not wrong, but overcommitted and undated, with the riskiest items (Preact,
oxlint, allow-net split) the least ready. Recommended framing: set a freeze
date, declare 2.9 the "package management + compile + test runner" release (with
Node 26.x compat as a headline), land those pillars, and punt JSX/Preact,
oxlint, and the allow-net split to 3.0 where per-package permissions already
live.

## Cross-references

- `TESTING_DOC.md` — node:test + mocking deep dive and W1-W6 plan.
- `docs/designs/testing_mock_core.md` — shared VirtualClock / mock core design.
