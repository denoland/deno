# Cutting a Deno release

**During this process `main` branch (or any other branch that you're creating
release from) should be frozen and no commits should land until the release is
cut.**

1. Create a PR that bumps versions of all crates in `op_crates` and `runtime`
   directories.

To determine if you should bump a crate a minor version instead of a patch
version, check if you can answer any of the following questions with yes:

- Did any of the crates direct dependencies have a semver breaking change? For
  example did we update swc_ecmascript from 0.56.0 to 0.57.0, or did we update
  rusty_v8?
- Did the external interface of the crate change (ops or changes to
  `window.__bootstrap` in JS code)?

When in doubt always do a minor bump instead of a patch. In essentially every
release all crates will need a minor bump. Patch bumps are the exception, not
the norm.

2. Make sure CI pipeline passes.

3. Publish all bumped crates to `crates.io`

**Make sure that `cargo` is logged on with a user that has permissions to
publish those crates.**

This is done by running `cargo publish` in each crate, because of dependencies
between the crates, it must be done in specific order:

- `deno_core` - all crates depend on `deno_core` so it must always be published
  first
- crates in `op_crates/` directory - there is no specific order required for
  those
- `runtime` - this crate depends on `deno_core` and all crates in `op_crates/`
  directory

If there are any problems when you publish, that require you to change the code,
then after applying the fixes they should be commited and pushed to the PR.

4. Once all crates are published merge the PR.

5. Create a PR that bumps `cli` crate version and updates `Releases.md`.

6. Make sure CI pipeline passes.

7. Publish `cli` crate to `crates.io`

8. Merge the PR.

9. Create a tag with the version number (with `v` prefix).

10. Wait for CI pipeline on the created tag branch to pass.

The CI pipeline will create a release draft on GitHub
(https://github.com/denoland/deno/releases).

11. Upload Apple M1 build to the release draft & to dl.deno.land.

12. Publish the release on Github

13. Update the Deno version on the website by updating
    https://github.com/denoland/deno_website2/blob/main/versions.json.
