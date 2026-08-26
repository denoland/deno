# Package management: `deno add` / `deno install`

This is the map for how Deno adds dependencies to a project: how the CLI flags
are parsed, how the configuration file is rewritten, and how packages are
actually installed into `node_modules` and the lockfile. Read this before
touching `deno add`, `deno install <pkg>`, or the flags that control where a
dependency is written (`--dev`, `--save-optional`, `--no-save`, `--save-exact`,
`--package-json`).

## The two flag parsers

Flag parsing currently lives in **two** places and both must be kept in sync
(the second is replacing the first, see the CLI-parser-split work):

- `cli/args/flags.rs` — the legacy `clap`-based parser. The `add` subcommand is
  defined in `add_subcommand()`; `install` reuses the shared argument builders
  (`add_dev_arg()`, `add_optional_arg()`, `add_no_save_arg()`). Both funnel into
  `add_parse_inner()`, which builds the `AddFlags` struct.
- `libs/cli_parser/` — the newer hand-written parser. Command shape is declared
  in `src/defs.rs` (`ADD_SUBCOMMAND`, `INSTALL_SUBCOMMAND`) and converted to
  flags in `src/convert.rs` (`add_parse`, and the install branch that produces
  `InstallFlagsLocal::Add`).

`AddFlags` itself is defined in `libs/cli_parser/src/flags.rs` and re-exported
through `crate::args`. When you add a field, you must update: the struct, both
parsers, and every `AddFlags { .. }` literal (there are literals in the parser
test suites `cli/args/flags.rs` and `libs/cli_parser/src/tests_full.rs`; the
shared test case is `add_or_install_subcommand`, which loops over both `add` and
`install`).

There is a lint (`ensureNoNonPermissionCapitalLetterShortFlags` in
`tools/lint.js`) that forbids capital-letter short flags unless they are on an
explicit allowlist with a documented precedent. `-D` (dev) and `-O`
(save-optional) are on it, both justified by the matching `npm install` short
flags.

## Where a dependency gets written: the config writer

`cli/tools/pm/mod.rs` is the core. The `add()` entry point resolves each
requested package to a concrete version
(`find_package_and_select_version_for_req`) and then decides which config file
to touch.

Two config files can be in play: `deno.json` (writes to `imports`) and
`package.json` (writes to a dependency section). `load_configs()` discovers
them, and — importantly — will _create_ one if none exists, because Deno needs a
config to manage `node_modules`. `prefer_npm_config` / `--package-json` /
`preferPackageJson` decide which one an npm package lands in when both exist.

The actual rewrite is `ConfigUpdater::add(selected, kind)`. `kind` is a
`DependencyKind` enum (`Normal` / `Dev` / `Optional`):

- `deno.json`: `kind` is ignored — everything goes under `imports`.
- `package.json`: `kind` selects the section (`dependencies`, `devDependencies`,
  `optionalDependencies`). `add()` also removes the package from the other two
  sections so it is never declared twice, and `new_dependency_section_index()`
  inserts a newly created section in a stable order (`dependencies` →
  `devDependencies` → `optionalDependencies`).

`ConfigUpdater::remove()` mirrors this and cleans all three sections.

## How packages actually get installed

After the config is (optionally) rewritten and committed, `add()` calls
`npm_install_after_modification()`, which builds a fresh `CliFactory` (to pick
up the edited config from disk) and calls `cache_deps::cache_top_level_deps()`.

`cache_top_level_deps()` (in `cli/tools/pm/cache_deps.rs`) is the shared install
routine used by `add`, `remove`, `install`, `outdated`, `audit`, `x`, etc. Its
model is: derive the set of graph roots from the project's **import map**
(deno.json imports) and its **package.json dependencies**, build the module
graph with npm resolution, then `cache_packages()` materializes everything into
`node_modules`.

### Gotcha: `optionalDependencies` are never installed from `package.json`

The installer only sees `dependencies` and `devDependencies`. This is a
limitation of the external `deno_package_json` crate: `PackageJsonDeps` /
`resolve_local_package_json_deps()` expose only those two maps — there is no
`optional_dependencies` in the resolved deps used by the installer. So a package
written to `optionalDependencies` will **not** be materialized by the normal
install path, even on a plain `deno install`.

To keep `--save-optional` at parity with `--save-dev` (which _does_ install on
add), `add()` installs optional packages directly instead of relying on the
config-derived roots. `CacheTopLevelDepsOptions` has an
`additional_roots:
Vec<Url>` field: any specifier put there is added to the
graph roots and installed regardless of whether it appears in the config.
`add()` populates it for both `--save-optional` and `--no-save` (see below). A
proper fix — teaching the installer to honor `optionalDependencies` — is a
separate, cross-crate change and is not done yet.

## The flags, end to end

- `--dev` / `-D`: `DependencyKind::Dev`. Writes to `devDependencies`, installs
  normally (dev deps are in the config-derived roots).
- `--save-optional` / `-O`: `DependencyKind::Optional`. Writes to
  `optionalDependencies`; because the installer ignores that section, the
  package is also pushed to `additional_roots` so it is installed on add.
- `--no-save`: resolve and install the package into `node_modules` and the
  lockfile, but do **not** rewrite or commit any config file. Implemented by
  skipping the `ConfigUpdater::add`/`commit` calls and pushing the package to
  `additional_roots`.
- These three are mutually exclusive (enforced with `conflicts_with` in both
  parsers).

`additional_roots` also flows through `cache_top_level_deps()`: the graph-build
section now runs when there is either an import map _or_ additional roots, so
`--no-save` works even in a package.json-only project with no import map.

## Tests

- Spec tests live under `tests/specs/add/`. Relevant ones: `dev/`,
  `save_optional/`, `no_save/`, `package_json_flag/`, `exiting_dev_deps/`. Run a
  subset with `./x test-spec add::`. A spec test that only needs the config
  result (not the download noise) uses `"output": "[WILDCARD]"` for the `add`
  step and asserts the file contents in a following `eval` step.
- Parser unit tests: `add_or_install_subcommand` in both `cli/args/flags.rs`
  (run via `cargo test -p deno --lib add_or_install`) and
  `libs/cli_parser/src/tests_full.rs`
  (`cargo test -p deno_cli_parser
  add_or_install`).

## Related work / next steps

- Make the installer honor `optionalDependencies` from `package.json` (upstream
  `deno_package_json` `resolve_local_package_json_deps` + the npm installer),
  which would let `--save-optional` install through the normal config-derived
  path and remove the `additional_roots` workaround for it.
- `--no-save` in a directory with no config still creates an empty config file
  (a side effect of `load_configs`), which slightly contradicts "no save"; the
  common case (existing project) is unaffected.
