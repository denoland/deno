- [ ] Fork this gist and follow the instructions there.

## Pre-flight

**During this process $BRANCH_NAME branch should be frozen and no commits should
land until the release is finished.**

- [ ] Ensure forks and local clones of:
  - [`denoland/deno`](https://github.com/denoland/deno/),
  - [`denoland/dotcom`](https://github.com/denoland/dotcom/),
  - [`denoland/deno_docker`](https://github.com/denoland/deno_docker/),
  - [`denoland/deno-docs`](https://github.com/denoland/deno-docs)
- [ ] Check https://deno.land/benchmarks?-100 and ensure there's no recent
      regressions.
- [ ] Write a message in company's `#cli` channel:

```
:lock: 

@here

Deno v$VERSION is now getting released.

`denoland/deno` is now locked.

*DO NOT LAND ANY PRs*

Release checklist: <LINK TO THIS FORKED GIST GOES HERE>
```

## Updating `deno`

### Phase 1: Bumping versions

- [ ] Go to the "version_bump" workflow in the CLI repo's actions:
      https://github.com/denoland/deno/actions/workflows/version_bump.yml
  1. Click on the "Run workflow" button.
  1. In the drop down, select the `main` branch.
  1. For the kind of release, select either `patch` or `minor`.
  1. Run the workflow.

- [ ] Wait for the workflow to complete and for a pull request to be
      automatically opened. Review the pull request, make any necessary changes,
      and merge it.
  - ⛔ **DO NOT** create a release tag manually That will automatically happen.

  <details>
     <summary>Failure Steps</summary>

  1. Checkout the branch the release is being made on.
  2. Manually run `./tools/release/01_bump_crate_versions.ts`
     1. Ensure the crate versions were bumped correctly
     2. Ensure `Releases.md` was updated correctly
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
     1. Checkout the `v$MINOR_VERSION` branch.
     2. If `cargo publish` hasn't completed then run
        `./tools/release/03_publish_crates.ts`
        - Note that you will need access to crates.io so it might fail.
     3. If `cargo publish` succeeded and a release tag wasn't created, then
        manually create and push the `v$VERSION` tag on the `v$MINOR_VERSION`
        branch.
  </details>

- [ ] This CI run create a tag which triggers a second CI run that publishes the
      GitHub draft release.

  The CI pipeline will create a release draft on GitHub
  (https://github.com/denoland/deno/releases).

- ⛔ Verify that:
  - [ ] There are 24 assets on the v$VERSION
        [GitHub release draft](https://github.com/denoland/deno/releases/).
  - [ ] There are 25 zip files for this version on
        [dl.deno.land](https://console.cloud.google.com/storage/browser/dl.deno.land/release/v$VERSION).

- [ ] Publish the release on Github

## Update https://deno.com

- [ ] Run
      https://github.com/denoland/dotcom/actions/workflows/update_version.yml to
      automatically open a PR.
  - [ ] Merge the PR.

## Update https://docs.deno.com

- [ ] Run
      https://github.com/denoland/deno-docs/actions/workflows/update_versions.yml
      to automatically open a PR.
  - [ ] Merge the PR.

## Updating `deno_docker`

- [ ] Run the version bump workflow:
      https://github.com/denoland/deno_docker/actions/workflows/version_bump.yml
- [ ] This will open a PR. Review and merge it.
- [ ] Create a `$VERSION` tag (_without_ `v` prefix).
- [ ] This will trigger a publish CI run. Verify that it completes sucessfully.

## Update MDN

- [ ] If a new JavaScript or Web API has been added or enabled, make sure
      https://github.com/mdn/browser-compat-data has been updated to reflect API
      changes in this release. If in doubt message @bartlomieju and skip this
      step.

## Add `deno upgrade` banner

- [ ] You can optionally add a banner that will be printed when users run
      `deno
      upgrade`. This is useful in situation when you want to inform
      users about a need to run a command to enjoy a new feature or a breaking
      change.
  - Create `banner.txt` file with the content you want to print - _it must be
    plaintext_.
  - Run
    `gsutil -h "Cache-Control: public, max-age=3600" cp banner.txt gs://dl.deno.land/release/v$VERSION/banner.txt`

## All done!

- [ ] Write a message in company's #cli channel:

```
:unlock:

@here 

`denoland/deno` is now unlocked.

*You can land PRs now*

Deno v$VERSION has been released.
```

## Downgrading

In case something went wrong:

1. Update https://dl.deno.land/release-latest.txt to the previous release.
1. Revert the PR to the [dotcom repo](https://github.com/denoland/dotcom/) in
   order to prevent the [`setup-deno`](https://github.com/denoland/setup-deno)
   GH action from pulling it in.
