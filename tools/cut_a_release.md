# Cutting a Deno release

**During this process `main` branch (or any other branch that you're creating
release from) should be frozen and no commits should land until the release is
cut.**

## Updating `deno_std`

1. Open a PR on the `deno_std` repo that bumps the version in `version.ts` and
   updates `Releases.md`

2. Before merging the PR, make sure that all tests pass when run using binary
   produced from bumping crates (point 3. from below).

3. Create a tag with the version number (_without_ `v` prefix).

## Updating the main repo

1. Run `./tools/release/01_bump_dependency_crate_versions.ts` to increase the
   minor versions of all crates in the `bench_util`, `core`, `ext`, and
   `runtime` directories.

2. Create a PR for this change.

3. Make sure CI pipeline passes (DO NOT merge yet).

4. Run `./tools/release/02_publish_dependency_crates.ts` to publish these bumped
   crates to `crates.io`

   **Make sure that `cargo` is logged on with a user that has permissions to
   publish those crates.**

   If there are any problems when you publish, that require you to change the
   code, then after applying the fixes they should be committed and pushed to
   the PR.

5. Once all crates are published merge the PR.

6. Run `./tools/release/03_bump_cli_version.ts` to bump the CLI version.

7. Use the output of the above command to update `Releases.md`

8. Create a PR for these changes.

9. Make sure CI pipeline passes.

10. Publish `cli` crate to `crates.io`

11. Merge the PR.

12. Create a tag with the version number (with `v` prefix).

13. Wait for CI pipeline on the created tag branch to pass.

The CI pipeline will create a release draft on GitHub
(https://github.com/denoland/deno/releases).

11. Upload Apple M1 build to the release draft & to dl.deno.land.

12. Publish the release on Github

13. Update the Deno version on the website by updating
    https://github.com/denoland/deno_website2/blob/main/versions.json.

14. Push a new tag to [`manual`](https://github.com/denoland/manual). The tag
    must match the tag from point 9; you don't need to create dedicated commit
    for that purpose, it's enough to tag the latest commit in that repo.

## Updating `deno_docker`

1. Open a PR on the `deno_docker` repo that bumps the Deno version in all
   Dockerfiles, the README and the example Dockerfile
2. Create a tag with the version number (_without_ `v` prefix).
