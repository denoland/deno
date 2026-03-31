- [ ] Fork this gist and follow the instructions there.

## Updating `deno`

### Phase 1: Bumping versions

- [ ] Go to the "version_bump" workflow in the CLI repo's actions:
      https://github.com/denoland/deno/actions/workflows/version_bump.yml
  1. Click on the "Run workflow" button.
  1. In the drop down, select the `main` branch.
  1. For the kind of release, select `alpha`, `beta`, or `rc`.
  1. Run the workflow.

- [ ] Wait for the workflow to complete and for a pull request to be
      automatically opened. Review the pull request, make any necessary changes,
      and merge it.
  - ⛔ **DO NOT** create a release tag manually. That will automatically happen.

  <details>
     <summary>Failure Steps</summary>

  1. Checkout the branch the release is being made on.
  2. Manually run `./tools/release/01_bump_crate_versions.ts --alpha` (or
     `--beta` / `--rc`)
  3. Open a PR with the changes and continue with the steps below.
  </details>

### Phase 2: Create tag

- [ ] Go to the "create_prerelease_tag" workflow in the CLI repo's actions:
      https://github.com/denoland/deno/actions/workflows/create_prerelease_tag.yml
  1. Run it on the same branch that you used before and wait for it to complete.

  <details>
     <summary>Failure Steps</summary>

  1. The workflow was designed to be restartable. Try restarting it.
  2. If that doesn't work, then manually create and push the `v$VERSION` tag.
  </details>

- [ ] This CI run creates a tag which triggers a second CI run that publishes
      the GitHub draft release.

  The CI pipeline will create a release draft on GitHub
  (https://github.com/denoland/deno/releases).

- ⛔ Verify that:
  - [ ] There are 28 assets on the v$VERSION
        [GitHub release draft](https://github.com/denoland/deno/releases/).

- [ ] Publish the release on Github

## Updating `deno_docker`

- [ ] Run the version bump workflow:
      https://github.com/denoland/deno_docker/actions/workflows/version_bump.yml
- [ ] This will open a PR. Review and merge it.
- [ ] Create a `$VERSION` tag (_without_ `v` prefix).
- [ ] This will trigger a publish CI run. Verify that it completes successfully.
