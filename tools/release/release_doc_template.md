- [ ] Fork this gist and follow the instructions there.

## Pre-flight

- Forks and local clones of
  [`denoland/deno`](https://github.com/denoland/deno/),
  [`denoland/deno_std`](https://github.com/denoland/deno_std/),
  [`denoland/dotcom`](https://github.com/denoland/dotcom/),
  [`denoland/deno_docker`](https://github.com/denoland/deno_docker/),
  [`denoland/deno-docs`](https://github.com/denoland/deno-docs)

**During this process `main` branch (or any other branch that you're creating
release from) should be frozen and no commits should land until the release is
cut.**

- [ ] Check https://deno.land/benchmarks?-100 and ensure there's no recent
      regressions.
- [ ] Write a message in company's #cli channel:
      `:lock: deno and deno_std are now locked (<LINK TO THIS FORKED GIST GOES HERE>)`

## Patch release preparation

**If you are cutting a patch release**: First you need to sync commits to the
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
    <summary>Failure Steps</summary>

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
    <summary>Failure Steps</summary>

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
  1. In the drop down, select the minor branch (ex. `vx.xx`) if doing a patch
     release or the main branch if doing a minor release.
  1. For the kind of release, select either "patch", "minor", or "major".
  1. Run the workflow.

- [ ] Wait for the workflow to complete and for a pull request to be
      automatically opened. Review the pull request, make any necessary changes,
      and merge it.
  - ⛔ DO NOT create a release tag manually That will automatically happen.

  <details>
     <summary>Failure Steps</summary>

  1. Checkout the branch the release is being made on.
  2. Manually run `./tools/release/01_bump_crate_versions.ts`
     1. Ensure the crate versions were bumped correctly
     2. Ensure deno_std version was updated correctly in `cli/deno_std.rs`
     3. Ensure `Releases.md` was updated correctly
  3. Open a PR with the changes and continue with the steps below.
  </details>

### Phase 2: Publish

- [ ] Go to the "cargo_publish" workflow in the CLI repo's actions:
      https://github.com/denoland/deno/actions/workflows/cargo_publish.yml
  1. Run it on the same branch that you used before and wait for it to complete.

  <details>
     <summary>Failure Steps</summary>

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
  (https://github.com/denoland/deno/releases).

- ⛔ Verify that:
  - [ ] There are 8 assets on the release draft.
  - [ ] There are 4 zip files for this version on
        [dl.deno.land](https://console.cloud.google.com/storage/browser/dl.deno.land/release/v$VERSION).

- [ ] Publish the release on Github

- [ ] Update https://github.com/denoland/dotcom/blob/main/versions.json and open
      a PR.
  - [ ] Merge the PR.

- [ ] Run
      https://github.com/denoland/deno-docs/actions/workflows/update_versions.yml
      to automatically open a PR.
  - [ ] Merge the PR.

- [ ] For minor releases: make sure https://github.com/mdn/browser-compat-data
      has been updated to reflect Web API changes in this release. Usually done
      ahead of time by @lucacasonato.

- [ ] **If you are cutting a patch release**: a PR should have been
      automatically opened that forwards the release commit back to main. If so,
      merge it. If not and it failed, please manually create one.

## Updating `deno.land/api` & `deno.land/std` symbols

This should occur after the Deno CLI & std are fully published, as the build
script generates the symbols based on the latest tags.

- [ ] Run the release workflow in the apiland_scripts repo on the main branch:
      https://github.com/denoland/apiland_scripts/actions/workflows/release.yml
  - [ ] Verify the workflow ran successfully.

  <details>
     <summary>Failure Steps</summary>

  1. Clone `deno/apiland_scripts`.
  2. Execute `deno task release`.
  </details>

## Updating `deno_docker`

- [ ] Run the version bump workflow:
      https://github.com/denoland/deno_docker/actions/workflows/version_bump.yml
- [ ] This will open a PR. Review and merge it.
- [ ] Create a tag with the version number (_without_ `v` prefix).

## All done!

- [ ] Write a message in company's #cli channel:
      `:unlock: deno and deno_std are now unlocked`.
