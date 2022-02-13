# Cutting a Deno release

## Pre-flight checklist

- [ ] An up to date stable Rust toolchain
- [ ] A binary version of `deno` available (hopefully built from `main`) that is
      going to be available throughout any local building you might do.
- [ ] Forks and local clones of
      [`denoland/deno`](https://github.com/denoland/deno/),
      [`denoland/deno_std`](https://github.com/denoland/deno_std/),
      [`denoland/dotland`](https://github.com/denoland/dotland/),
      [`denoland/docland`](https://github.com/denoland/docland/),
      [`denoland/deno_docker`](https://github.com/denoland/deno_docker/)
      [`denoland/manual`](https://github.com/denoland/manual/)
- [ ] Ensure that external dependencies are up-to date in `denoland/deno` (e.g.
      `rusty_v8`, `serde_v8`, `deno_doc`, `deno_lint`).
- [ ] Ownership access on crates.io for the 19 (🙀) crates that you will be
      publishing. (Don't worry too much though as the main script publishing 18
      of the crates allows recovery)
- [ ] Lot's of ☕

**During this process `main` branch (or any other branch that you're creating
release from) should be frozen and no commits should land until the release is
cut.**

Before starting the process write a message in company's #general channel:
`:lock: deno and deno_std are now locked`

## Updating `deno_std`

1. Checkout a branch for releasing `std` (e.g. `release_#.#.#`).

2. Open a PR on the `deno_std` repo that bumps the version in `version.ts` and
   updates `Releases.md`. You can use following command to generate a short list
   that needs to be updated: `git log --oneline <previous_tag>..` (replace
   `<previous_tag>` with actual latest tag, eg. `git log --oneline 0.122.0..`).
   Remove all commits that are not `feat` or `fix`.

3. Before merging the PR, make sure that all tests pass when run using binary
   produced from bumping crates (point 3. from below).

4. When merging the PR, ensure that the commit name is exactly the version name.
   Eg. `0.121.0`, not `0.121.0 (#1810)`.

5. Pull the latest `main` branch and make sure the commit from the merged PR is
   there. Create a tag with the version number (_without_ `v` prefix), eg.
   `deno tag 0.122.0 <commit_hash>`, then push the tag to the
   `denoland/deno_std` repository, eg. `git push upstream 0.122.0`.

6. Once CI passes, copy contents of `Releases.md` you added, and create a new
   release on GitHub (https://github.com/denoland/deno_std/releases).

## Updating the main repo

**If you are cutting a patch release**: First you need to sync commit to the
relevant minor branch, so if you are cutting a `v1.17.3` release you need to
sync `v1.17` branch.

To do that, you need to cherry-pick commits from the main branch to the `v1.17`
branch. For patch releases we want to cherry-pick all commits that are not
`feat` commits. Check what was the last commit on `v1.17` branch before the
previous release and start cherry-picking newer commits from the `main`.

Once all relevant commits are cherry-picked, push the branch to the upstream and
verify on GitHub that everything looks correct.

1. Checkout a branch for releasing crate dependencies (e.g. `deps_#.#.#`).

2. Run `./tools/release/01_bump_dependency_crate_versions.ts` to increase the
   minor versions of all crates in the `bench_util`, `core`, `ext`, and
   `runtime` directories.

3. Commit these changes with a commit message like
   `chore: bump crate version for #.#.#` and create a PR for this change. **If
   you are cutting a patch release**: make sure to target `v1.XX` branch instead
   of `main` in your PR.

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

10. Use the output of the above command to update `Releases.md`. **If you are
    cutting a minor release**: make sure that there are no duplicate entries in
    previous releases; most often commits with `fix` prefix would have been
    included in patch releases.

11. Update link in `cli/compat/mod.rs` with the released version of `deno_std`
    and do a search through the tests to find std urls that need to be updated.

12. Create a PR for these changes. **If you are cutting a patch release**: make
    sure to target `v1.XX` branch instead of `main` in your PR.

13. Make sure CI pipeline passes.

14. Publish `cli` crate to `crates.io`: `cd cli && cargo publish`

15. Merge the PR.

16. Create a tag with the version number (with `v` prefix).

17. Wait for CI pipeline on the created tag branch to pass.

    The CI pipeline will create a release draft on GitHub
    (https://github.com/denoland/deno/releases).

18. Upload Apple M1 build (`deno-aarch64-apple-darwin.zip`) to the release draft
    and to https://console.cloud.google.com/storage/browser/dl.deno.land

    ```
    cargo build --release
    cd target/release
    zip -r deno-aarch64-apple-darwin.zip deno
    ```

19. Publish the release on Github

20. Update the Deno version on the website by updating
    https://github.com/denoland/dotland/blob/main/versions.json.

21. Push a new tag to [`manual`](https://github.com/denoland/manual). The tag
    must match the CLI tag; you don't need to create dedicated commit for that
    purpose, it's enough to tag the latest commit in that repo.

22. For minor releases: make sure https://github.com/mdn/browser-compat-data has
    been updated to reflect Web API changes in this release. Usually done ahead
    of time by @lucacasonato.

23. **If you are cutting a patch release**: open a PR that forwards all commits
    created in the release process to the `main` branch.

## Updating `doc.deno.land`

This should occur after the Deno CLI is fully published, as the build script
queries the GitHub API to determine what it needs to change and update.

1. Goto the cloned report for `denoland/docland`.

2. Checkout a new branch (e.g. `git checkout -b deno_1.17.0`).

3. Execute `./build.ts` (or
   `deno run --config deno.jsonc --import-map import-map.json --allow-read=. --allow-write=./static --allow-net build.ts`).

4. Commit changes and raise a PR on `denoland/docland`.

5. Merging the approved PR will trigger deployment to Deploy of the updates.

## Updating `deno_docker`

1. Open a PR on the `deno_docker` repo that bumps the Deno version in all
   Dockerfiles, the README and the example Dockerfile
2. Create a tag with the version number (_without_ `v` prefix).

Write a message in company's #general channel:
`:unlock: deno and deno_std are now unlocked`.
