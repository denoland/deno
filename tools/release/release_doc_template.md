## Pre-flight checklist

- Forks and local clones of
  [`denoland/deno`](https://github.com/denoland/deno/),
  [`denoland/deno_std`](https://github.com/denoland/deno_std/),
  [`denoland/dotland`](https://github.com/denoland/dotland/),
  [`denoland/docland`](https://github.com/denoland/docland/),
  [`denoland/deno_docker`](https://github.com/denoland/deno_docker/)
  [`denoland/manual`](https://github.com/denoland/manual/)

**During this process `main` branch (or any other branch that you're creating
release from) should be frozen and no commits should land until the release is
cut.**

- [ ] Write a message in company's #cli channel:
      `:lock: deno and deno_std are now locked (<LINK TO THIS GIST GOES HERE>)`

## Patch release preparation

**If you are cutting a patch release**: First you need to sync commit to the
relevant minor branch in the `deno` repo, so if you are cutting a `v1.17.3`
release you need to sync `v1.17` branch.

To do that, you need to cherry-pick commits from the main branch to the `v1.17`
branch. For patch releases we want to cherry-pick all commits that do not add
features to the CLI. This generally means to filter out `feat` commits but not
necessarily (ex. `feat(core): ...`). Check what was the last commit on `v1.17`
branch before the previous release and start cherry-picking newer commits from
the `main`.

Once all relevant commits are cherry-picked, push the branch to the upstream and
verify on GitHub that everything looks correct.

- ⛔ DO NOT create a `vx.xx.x`-like branch! You are meant to cherry pick to a
  `vx.xx` branch. If you have accidentally created a `vx.xx.x`-like branch then
  delete it as tagging the CLI will fail otherwise.

- [ ] Unstable `feat` commits were merged.
- [ ] Internal API commits like `feat(core)` were merged.

## Updating `deno_std`

- [ ] Go to the "version_bump" workflow in the deno_std repo's actions:
      https://github.com/denoland/deno_std/actions/workflows/version_bump.yml
  1. Click on the "Run workflow" button.
  1. For the kind of release, select "minor".
  1. Run the workflow.

- [ ] A PR will be automatically created. Follow the checklist in the PR, review
      it, and merge the PR.
  - ⛔ DO NOT create a release tag manually. That will automatically happen.

  <details>
    <summary>❌ Failure Steps</summary>

  1. Checkout the latest main.
  2. Manually run `./_tools/release/01_bump_version.ts --minor`
     1. Ensure the version in `version.ts` is updated correctly.
     2. Ensure `Releases.md` is updated correctly.
     3. Ensure all the tests pass with the latest build (examine the repo for
        what the command is and run the local built deno binary)
  3. Open a PR with the changes and continue with the steps below.
  </details>

- Wait for the CI run to complete which will automatically tag the repo and
  create a draft release.
  - [ ] Review the draft release and then publish it.

  <details>
    <summary>❌ Failure Steps</summary>

  1. Tag the repo manually in the format `x.x.x`
  2. Draft a new GH release by copying and pasting the release notes from
     `Releases.md`
  </details>

## Updating `deno`

### Phase 1: Bumping versions

- [ ] After releasing deno_std, go to the "version_bump" workflow in the CLI
      repo's actions:
      https://github.com/denoland/deno/actions/workflows/version_bump.yml
  1. Click on the "Run workflow" button.
  1. In the drop down, select the minor branch if doing a patch release or the
     main branch if doing a minor release.
  1. For the kind of release, select either "patch", "minor", or "major".
  1. Run the workflow.

- [ ] Wait for the workflow to complete and for a pull request to be
      automatically opened. Review the pull request, make any necessary changes,
      and merge it.
  - ⛔ DO NOT create a release tag manually.

  <details>
     <summary>❌ Failure Steps</summary>

  1. Checkout the branch the release is being made on.
  2. Manually run `./tools/release/01_bump_crate_versions.ts`
     1. Ensure the crate versions were bumped correctly
     2. Ensure deno_std version was updated correctly in `cli/compat/mod.rs`
     3. Ensure `Releases.md` was updated correctly
  3. Open a PR with the changes and continue with the steps below.
  </details>

### Phase 2: Publish

- [ ] Go to the "cargo_publish" workflow in the CLI repo's actions:
      https://github.com/denoland/deno/actions/workflows/cargo_publish.yml
  1. Run it on the same branch that you used before and wait for it to complete.

  <details>
     <summary>❌ Failure Steps</summary>

  1. The workflow was designed to be restartable. Try restarting it.
  2. If that doesn't work, then do the following:
     1. Checkout the branch the release is occurring on.
     2. If `cargo publish` hasn't completed then run
        `./tools/release/03_publish_crates.ts`
        - Note that you will need access to crates.io so it might fail.
     3. If `cargo publish` succeeded and a release tag wasn't created, then
        manually create and push one for the release branch with a leading `v`.
  </details>

- [ ] This CI run create a tag which triggers a second CI run that publishes the
      GitHub draft release.

  The CI pipeline will create a release draft on GitHub
  (https://github.com/denoland/deno/releases). Update the draft with the
  contents of `Releases.md` that you previously added.

- [ ] Upload Apple M1 build (`deno-aarch64-apple-darwin.zip`) to the release
      draft and to https://console.cloud.google.com/storage/browser/dl.deno.land

  ```
  cargo build --release
  cd target/release
  zip -r deno-aarch64-apple-darwin.zip deno
  ```

- ⛔ Verify that:
  - [ ] There are 8 assets on the release draft.
  - [ ] There are 4 zip files for this version on dl.deno.land
  - [ ] The aarch64 Mac build was built from the correct branch AFTER the
        version bump and has the same version as the release when doing
        `deno -V` (ask someone with an M1 Mac to verify this if you don't have
        one).

- [ ] Publish the release on Github

- [ ] Run the
      https://github.com/denoland/dotland/actions/workflows/update_versions.yml
      workflow.
  - [ ] This should open a PR. Review and merge it.

  <details>
     <summary>❌ Failure Steps</summary>

  1. Update https://github.com/denoland/dotland/blob/main/versions.json
     manually.
  2. Open a PR and merge.
  </details>

- [ ] Push a new tag to [`manual`](https://github.com/denoland/manual). The tag
      must match the CLI tag; you don't need to create dedicated commit for that
      purpose, it's enough to tag the latest commit in that repo.

- [ ] For minor releases: make sure https://github.com/mdn/browser-compat-data
      has been updated to reflect Web API changes in this release. Usually done
      ahead of time by @lucacasonato.

- [ ] **If you are cutting a patch release**: a PR should have been
      automatically opened that forwards the release commit back to main. If so,
      merge it. If not and it failed, please manually create one.

## Updating `doc.deno.land`

This should occur after the Deno CLI is fully published, as the build script
queries the GitHub API to determine what it needs to change and update.

- [ ] Run the update_deno workflow in the docland repo on the main branch:
      https://github.com/denoland/docland/actions/workflows/update_deno.yml
  - This will open a PR. Review it and merge, which will trigger a deployment.

  <details>
     <summary>❌ Failure Steps</summary>

  1. Checkout a new branch for docland (e.g. `git checkout -b deno_1.17.0`).
  2. Execute `deno task build`
  3. Commit changes and raise a PR on `denoland/docland`.
  4. Merging the approved PR will trigger deployment to Deploy of the updates.
  </details>

## Updating `deno_docker`

- [ ] Open a PR on the `deno_docker` repo that bumps the Deno version in all
      Dockerfiles, the README and the example Dockerfile. Get it reviewed and
      merge it.
- [ ] Create a tag with the version number (_without_ `v` prefix).

## All done!

- [ ] Write a message in company's #general channel:
      `:unlock: deno and deno_std are now unlocked`.
