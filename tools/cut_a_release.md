# Cutting a Deno release

**During this process `main` branch (or any other branch that you're creating
release from) should be frozen and no commits should land until the release is
cut.**

## Updating `deno_std`

1. Checkout a branch for releasing `std` (e.g. `release_#.#.#`).

2. Open a PR on the `deno_std` repo that bumps the version in `version.ts` and
   updates `Releases.md`

3. Before merging the PR, make sure that all tests pass when run using binary
   produced from bumping crates (point 3. from below).

4. Create a tag with the version number (_without_ `v` prefix).

## Updating the main repo

1. Checkout a branch for releasing crate dependencies (e.g. `deps_#.#.#`).

2. Run `./tools/release/01_bump_dependency_crate_versions.ts` to increase the
   minor versions of all crates in the `bench_util`, `core`, `ext`, and
   `runtime` directories.

3. Commit these changes with a commit message like
   `chore: bump crate version for #.#.#` and create a PR for this change.

4. Make sure CI pipeline passes (DO NOT merge yet).

5. Run `./tools/release/02_publish_dependency_crates.ts` to publish these bumped
   crates to `crates.io`

   **Make sure that `cargo` is logged on with a user that has permissions to
   publish those crates.**

   If there are any problems when you publish, that require you to change the
   code, then after applying the fixes they should be committed and pushed to
   the PR.

6. Once all crates are published merge the PR.

7. Run `./tools/release/03_bump_cli_version.ts` to bump the CLI version.

8. Use the output of the above command to update `Releases.md`

9. Create a PR for these changes.

10. Make sure CI pipeline passes.

11. Publish `cli` crate to `crates.io`

12. Merge the PR.

13. Create a tag with the version number (with `v` prefix).

14. Wait for CI pipeline on the created tag branch to pass.

    The CI pipeline will create a release draft on GitHub
    (https://github.com/denoland/deno/releases).

15. Upload Apple M1 build to the release draft & to dl.deno.land.

16. Publish the release on Github

17. Update the Deno version on the website by updating
    https://github.com/denoland/deno_website2/blob/main/versions.json.

18. Push a new tag to [`manual`](https://github.com/denoland/manual). The tag
    must match the CLI tag; you don't need to create dedicated commit for that
    purpose, it's enough to tag the latest commit in that repo.

## Updating `deno_docker`

1. Open a PR on the `deno_docker` repo that bumps the Deno version in all
   Dockerfiles, the README and the example Dockerfile
2. Create a tag with the version number (_without_ `v` prefix).
