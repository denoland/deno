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

1. Create a PR that does a minor version bump of all crates in `bench_util`,
   `core`, `ext`, `runtime` directories.

2. Make sure CI pipeline passes.

3. Publish all bumped crates to `crates.io`

**Make sure that `cargo` is logged on with a user that has permissions to
publish those crates.**

This is done by running `cargo publish` in each crate, because of dependencies
between the crates, it must be done in specific order:

- `deno_core` - all crates depend on `deno_core` so it must always be published
  first
- `bench_util`
- crates in `ext/` directory, publish in the following order:
  - broadcast_channel
  - console
  - ffi
  - tls
  - web
  - webgpu
  - webidl
  - websocket
  - webstorage
  - crypto
  - fetch
  - http
  - net
  - url
  - timers
- `runtime` - this crate depends on `deno_core` and all crates in `ext/`
  directory

If there are any problems when you publish, that require you to change the code,
then after applying the fixes they should be committed and pushed to the PR.

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

14. Push a new tag to [`manual`](https://github.com/denoland/manual). The tag
    must match the tag from point 9; you don't need to create dedicated commit
    for that purpose, it's enough to tag the latest commit in that repo.

## Updating `deno_docker`

1. Open a PR on the `deno_docker` repo that bumps the Deno version in all
   Dockerfiles, the README and the example Dockerfile
2. Create a tag with the version number (_without_ `v` prefix).
