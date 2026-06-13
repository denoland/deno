# Research: `deno test --changed` / `--affected` (issue #28182)

Dependency-aware, git-driven test selection for `deno test`, working across
workspace members. This document captures what the feature actually needs to be,
prior art in other runners, what already exists in the Deno codebase, a
recommended design, and the concrete edge cases.

Tracking issue: <https://github.com/denoland/deno/issues/28182>

## 1. What is actually being asked for

The issue title says "git-based `deno test --changed`", but the discussion
narrows it considerably:

- A maintainer (marvinhagemeister) pointed out that `deno test --watch` **already**
  re-runs only the tests that import a changed module.
- The requester clarified: `--watch` is *continuous*, which is the wrong shape.
  They want a **single, on-demand, non-continuous** run that is dependency-aware:
  > "A command like `deno test --affected <changed_file>` would be ideal. It
  > would analyze dependencies ... and execute only the truly relevant tests
  > just once."

So the ask is: **bring the affected-test selection that already happens inside
watch mode to a one-shot run, sourcing the "changed files" from git (or an
explicit set) instead of from the file-watcher.** It should also work across a
workspace, where one member's tests depend on another member's source.

The existing third-party workaround (`@staytuned/deno-dag-test`) builds the
import DAG and walks it the same way Deno already does internally.

## 2. Prior art

| Tool | Flag(s) | Granularity | Change source | Graph mapping |
|------|---------|-------------|---------------|---------------|
| **Vitest** | `--changed [ref]`, `--related <files>` | file | no ref → uncommitted (staged+unstaged); `--changed HEAD~1` / hash / branch → that diff | Vite module graph; runs test files that (transitively) import a changed module |
| **Jest** | `-o`/`--onlyChanged`, `--changedSince <ref>`, `--changedFilesWithAncestor`, `--lastCommit` | file | `jest-changed-files` (git or hg); uncommitted ± last commit, or since a ref | Haste module map; tests depending on changed files |
| **Nx** | `nx affected -t test --base <ref> --head <ref>` | project | git diff base..head; default base = main branch, head = working tree | project graph: files → owning project → **dependent** projects |
| **Turborepo** | `turbo run test --filter=...[ref]`; `--affected` shorthand | package | git diff vs ref/range; `--affected` ≈ `...[main...HEAD]` (overridable via `TURBO_SCM_BASE`/`TURBO_SCM_HEAD`) | package graph: `[ref]` = changed only, `...[ref]` = changed + dependents, `[ref]...` = changed + deps |
| **Bazel** | (none native) `bazel-diff`, `target-determinator` | target | content/merkle hash of action graph between commits | transitive action-graph; remote cache skips unchanged test actions |
| **Pants** | `--changed-since=<ref>`, `--changed-dependents` | target | git diff since ref | dependee graph |
| **Gradle** | build cache + up-to-date checks | task | input fingerprints | test task skipped when inputs unchanged |

Sources: Vitest [CLI](https://vitest.dev/guide/cli) ·
[`--changed` discussion](https://github.com/vitest-dev/vitest/discussions/6734) ·
[`forceRerunTriggers`](https://vitest.dev/config/forcereruntriggers);
Jest [CLI](https://jestjs.io/docs/cli) ·
[`jest-changed-files`](https://www.npmjs.com/package/jest-changed-files);
Nx [Affected](https://nx.dev/ci/features/affected) ·
[nx-set-shas](https://github.com/nrwl/nx-set-shas);
Turborepo [filtering rules](https://github.com/vercel/turborepo/blob/main/skills/turborepo/references/filtering/RULE.md) ·
[`--affected` discussion](https://github.com/vercel/turborepo/discussions/9076).

### Common patterns

1. **Two inputs**: a set of changed files (from git) + a dependency graph.
2. **Granularity split**: monorepo *task* runners (Nx, Turbo) select at the
   project/package level; *test* runners (Jest, Vitest) select at the file level.
   `deno test` is a file-level test runner, so file-level selection is the right
   fit (and is what watch mode already does).
3. **Direction**: from a changed file, find the *dependents* (the tests that
   transitively import it). Two ways to implement: a reverse graph, or — as Deno
   already does — forward-walk each test root and check whether any of its deps
   is in the changed set.
4. **Git ref convention**: default to the **working-tree / uncommitted** diff for
   local dev; accept an explicit ref or `base...head` range for CI, typically
   resolved through a merge-base.
5. **Escape hatches are mandatory**: not every dependency is an import edge
   (config files, JSON/data fixtures read at runtime, env files, the test runner
   config itself). Every tool has a "force rerun everything" trigger
   (`forceRerunTriggers`, Nx `namedInputs`/implicit deps + global files, Turbo
   `globalDependencies`). Vitest reruns the whole suite when the config or
   `package.json` changes.
6. **Caching is a separate, complementary layer** (Turbo/Bazel/Gradle hash task
   inputs and skip cached results). The Deno issue is purely about *selection*,
   not result caching, so that layer is out of scope here.

## 3. What already exists in the Deno codebase

The machinery is essentially all there — it's wired to the file watcher rather
than to git.

- **Graph-walk selection primitive** —
  `cli/graph_util.rs:1137` `has_graph_root_local_dependent_changed(graph, root,
  changed_paths) -> bool`. Walks a test root's dependencies
  (`follow_dynamic: true`, skipping remote modules) and returns true if any
  local dependency is in the changed set. This is exactly the affected check.

- **Watch already does the selection** — `cli/tools/test/mod.rs:2131-2172`.
  In `run_tests_with_watch`, after building the graph it takes `changed_paths`
  from the watcher and filters test modules:
  - env-file change → reload everything (`mod.rs:2136`, an existing escape hatch);
  - otherwise keep each test module where `has_graph_root_local_dependent_changed`
    is true; doc-only (`.md`) modules are matched by path.
  This is the precise behavior to reuse — only the source of `changed_paths`
  differs.

- **One-shot entry point** — `cli/tools/test/mod.rs:1910` `run_tests()`. Collects
  specifiers via `fetch_specifiers_with_test_mode` (`mod.rs:1923`), typechecks,
  then `test_specifiers`. The graph is currently built inside the typecheck
  container; an affected filter would need a graph *before* run (build it with
  `module_graph_creator.create_graph`, as the watch path already does).

- **Workspace enumeration** — `cli/args/mod.rs` `resolve_test_options_for_members`
  (used at `mod.rs:1921`) already enumerates every workspace member's test files,
  and the module graph spans the whole workspace. So a change in member B's source
  that member A's test imports is just an edge in the same graph — cross-workspace
  affected selection is essentially **free** once changed paths are known. This is
  the key "works across workspaces" property the issue asks for.

- **Git plumbing precedent** — there is **no** git change-detection today, but
  `cli/tools/bump_version.rs:951` already shells out via `run_git(cwd, args)`
  (`Command::new("git")`), incl. `rev-parse --show-toplevel`, `rev-parse
  --abbrev-ref HEAD`, `show <ref>`. Same pattern works for `diff --name-only`.

- **No existing `--changed`/`--affected` flag.** `TestFlags`
  (`cli/args/flags.rs:627`) has `watch`, `filter`, `files`, etc., but nothing for
  change-based selection. Confirmed by grep + PR search.

## 4. Recommended design

### CLI surface

- `deno test --changed[=<ref>]` as the primary flag (matches the issue title and
  the Vitest/Jest mental model for a *test runner*). Semantics mirror Vitest:
  - no value → diff of the working tree (staged + unstaged + untracked) vs `HEAD`;
  - `=<ref>` (branch, tag, or commit) → `git diff --name-only <merge-base>...`
    plus the working-tree diff, so local edits on top of a branch are included.
- Accept an explicit file list too (the requester literally wrote
  `--affected <file>`): allow positional/`--related`-style files to seed the
  changed set without touching git. Useful in CI that already knows the diff.
  (Prior art: Vitest `--related <files>`, Jest `--findRelatedTests <files>`, Nx
  `--files=<csv>` all provide this git-free seed; Nx also has `--uncommitted` /
  `--untracked` toggles worth considering.)
- Compose with existing flags: `--changed` + `--watch` (initial filter, then
  normal watch), `--filter`, `--coverage`, workspace runs.
- Consider mirroring to `deno bench` later; keep this PR scoped to `test`.

`TestFlags`: add `pub changed: Option<ChangedSpec>` where `ChangedSpec` captures
"working tree" vs "since ref" vs "explicit files". (Using `Option<Option<String>>`
also works but an enum reads better.)

### Granularity & graph

File-level, reusing `has_graph_root_local_dependent_changed`. No reverse graph
needed.

### Flow (one-shot, in `run_tests`)

1. Collect candidate specifiers across all members (existing
   `fetch_specifiers_with_test_mode`).
2. If `--changed` is set:
   a. Resolve the changed-file set: from git (`git diff --name-only` +
      `git status --porcelain` for untracked, merge-base for a ref) or from the
      explicit file list; canonicalize paths.
   b. Build the graph over the candidate roots
      (`module_graph_creator.create_graph`, as watch does at `mod.rs:2125`).
   c. Keep a candidate if it *is* a changed file, or
      `has_graph_root_local_dependent_changed(graph, specifier, changed)` is true;
      match doc-only modules by path (same split as `mod.rs:2118`).
   d. Apply escape hatches → fall back to running everything (see §5).
3. Typecheck + run the filtered set. If empty, respect `permit_no_files`
   (exit 0 with a clear "no affected test files" message rather than erroring).

### Where the code lands

1. `cli/args/flags.rs` — add the flag to the `test` subcommand + `TestFlags`
   field + parsing.
2. New helper, e.g. `cli/util/git.rs` (or `cli/tools/test/changed.rs`):
   `changed_files(cwd, base: Option<&str>) -> Result<HashSet<PathBuf>>`, reusing
   the `run_git` pattern. Resolve repo root, merge-base, name-only diff, untracked.

   The git commands can be lifted almost verbatim from Vitest/Jest (both
   verified against their current source) — note the **three-dot** range, which
   diffs against the *merge-base* so local edits on top of a branch are included
   and upstream commits on the ref are not:

   ```text
   # repo root
   git rev-parse --show-cdup            # (or --show-toplevel, as bump_version.rs uses)

   # bare --changed  → working-tree (staged + unstaged + untracked)
   git diff --cached --name-only
   git ls-files --other --modified --exclude-standard   # --other adds untracked; respects .gitignore

   # --changed=<ref> → above, plus committed changes since the merge-base
   git diff --name-only <ref>...HEAD    # three-dot = since merge-base(<ref>, HEAD)
   ```

   Mirror Vitest/Jest exactly here: bare flag = uncommitted working tree,
   `=<ref>` adds the merge-base diff. `--exclude-standard` keeps ignored files
   out.
3. `cli/tools/test/mod.rs` — build graph + filter in `run_tests` before
   typecheck; apply the same seeding to `run_tests_with_watch` so `--changed`
   composes with `--watch`.
4. Spec tests under `tests/specs/test/` (a git fixture repo with members) +
   docs in `cli/args/flags.rs` help text.

## 5. Edge cases & decisions

- **Non-import dependencies** (deno.json(c) / import map, JSON & data fixtures
  read via `Deno.readTextFile`, `.env`): invisible to the import graph. This is
  the single biggest design decision, and prior art splits two ways:
  - **Jest selects *nothing*** for an unmappable change (no run-everything
    fallback) — a well-known CI footgun where editing `jest.config.js` runs zero
    tests.
  - **Vitest reruns *everything*** when a change matches `forceRerunTriggers`
    (default globs: `**/package.json`, `**/{vitest,vite}.config.*`).
  Recommendation: follow Vitest. Mirror the existing env-file escape hatch
  (`mod.rs:2136`): a change to `deno.json`/`deno.jsonc` / import map / `deno.lock`
  / `.env` → run the whole (workspace) suite. Optionally expose a Vitest-style
  `forceRerunTriggers` glob in `deno.json` for project-specific fixtures.
- **A changed file that is itself a test file** → always select it.
- **Newly added test file** → no prior graph; treat "is in changed set" as select.
- **Deleted file** → cannot be a graph node. Two options: drop it from the
  changed set (its former dependents fail typecheck anyway, surfacing the
  breakage — simplest), or, like Nx (which assumes *all* projects affected when a
  project is deleted), fall back to running everything. Recommend the drop
  approach for files; reserve the run-all fallback for the escape-hatch configs.
- **Dynamic / non-analyzable imports** → graph may miss them; `follow_dynamic:
  true` is already set, but document that fully dynamic specifiers can be missed.
- **Remote (npm:/jsr:/https:) changes** → only local `file:` changes matter; the
  helper already skips remote subtrees.
- **Not a git repo / git missing / shallow clone** → Jest is famously a "courier
  for git errors" here. Fail with an actionable message (and a hint about CI
  shallow-clone / fetch-depth), or allow `--changed=<explicit files>` to bypass
  git entirely.
- **Default base for CI** → unlike Nx/Turbo we should *not* silently default to
  `main`; default to the working tree (local-dev shape the requester wants) and
  require an explicit `--changed=<ref>` for base..head CI comparisons. Document
  the `origin/main` / merge-base recipe.
- **Listing** → consider letting `--no-run` (or a `list`-like mode) print the
  selected files, paralleling `vitest list --changed`, for debuggability.

## 6. Effort estimate

Small-to-medium. The selection algorithm, graph construction, workspace
enumeration, and doc-module handling already exist and are battle-tested in watch
mode. The genuinely new work is: (1) flag plumbing, (2) a ~50-line git
change-detection helper, (3) lifting the watch-mode filter into the one-shot
`run_tests` path, and (4) the escape-hatch policy for non-import dependencies.
The cross-workspace requirement needs no special handling — it falls out of the
single workspace-wide module graph.
