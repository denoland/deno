# Cutting a Deno release

## Pre-flight checklist

- [ ] An up to date stable Rust toolchain
- [ ] A binary version of `deno` available (hopefully built from `main`) that is
      going to be available throughout any local building you might do.
- [ ] Forks and local clones of
      [`denoland/deno`](https://github.com/denoland/deno/),
      [`denoland/deno_std`](https://github.com/denoland/deno_std/),
      [`denoland/dotland`](https://github.com/denoland/dotland/) and
      [`denoland/deno_docker`](https://github.com/denoland/deno_docker/)
- [ ] Ensure that external dependencies are up-to date in `denoland/deno` (e.g.
      `rusty_v8`, `serde_v8`, `deno_doc`, `deno_lint`).
- [ ] Ownership access on crates.io for the 19 (🙀) crates that you will be
      publishing. (Don't worry too much though as the main script publishing 18
      of the crates allows recovery)
- [ ] Lot's of ☕

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

7. Update your main branch and checkout another branch (e.g. `release_#.#.#`).

8. Run `./tools/release/03_bump_cli_version.ts` to bump the CLI version.

9. If you are doing a patch release, answer `y` to the _Increment patch?_
   prompt.

10. Use the output of the above command to update `Releases.md`

11. Update link in `cli/compat/mod.rs` with the released version of `deno_std`
    and do a search through the tests to find std urls that need to be updated.

12. Create a PR for these changes.

13. Make sure CI pipeline passes.

14. Publish `cli` crate to `crates.io`

15. Merge the PR.

16. Create a tag with the version number (with `v` prefix).

17. Wait for CI pipeline on the created tag branch to pass.

    The CI pipeline will create a release draft on GitHub
    (https://github.com/denoland/deno/releases).

18. Upload Apple M1 build to the release draft & to dl.deno.land.

19. Publish the release on Github

20. Update the Deno version on the website by updating
    https://github.com/denoland/dotland/blob/main/versions.json.

21. Push a new tag to [`manual`](https://github.com/denoland/manual). The tag
    must match the CLI tag; you don't need to create dedicated commit for that
    purpose, it's enough to tag the latest commit in that repo.

22. For minor releases: make sure https://github.com/mdn/browser-compat-data has
    been updated to reflect Web API changes in this release. Usually done ahead
    of time by @lucacasonato.

## Updating `deno_docker`

1. Open a PR on the `deno_docker` repo that bumps the Deno version in all
   Dockerfiles, the README and the example Dockerfile
2. Create a tag with the version number (_without_ `v` prefix).
