# Cutting a Deno release

**During this process `main` branch should be frozen and no commits should land
until the release is cut.**

1. Create a PR that bumps versions of all crates in `op_crates` and `runtime`
   directories.

If this is a Deno patch release then the crates should have patch version bumped
as well; analogously for a minor Deno release the crates should have minor
version bumped.

2. Make sure CI pipeline passes.

3. Publish all bumped crates to `crates.io`

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

9. Wait for CI pipeline on `main` branch to pass.

It will create a release draft on GitHub
(https://github.com/denoland/deno/releases).

10. Upload Apple M1 build to the release draft.

11. Publish the release on Github
