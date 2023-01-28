#!/usr/bin/env -S deno run --allow-write=. --lock=./tools/deno.lock.json
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as yaml from "https://deno.land/std@0.173.0/encoding/yaml.ts";

const windowsRunnerCondition =
  "github.repository == 'denoland/deno' && 'windows-2022-xl' || 'windows-2022'";
const Runners = {
  linux:
    "${{ github.repository == 'denoland/deno' && 'ubuntu-20.04-xl' || 'ubuntu-20.04' }}",
  macos: "macos-12",
  windows: `\${{ ${windowsRunnerCondition} }}`,
};

const installPkgsCommand =
  "sudo apt-get install --no-install-recommends debootstrap clang-15 lld-15";
const sysRootStep = {
  name: "Set up incremental LTO and sysroot build",
  run: `# Avoid running man-db triggers, which sometimes takes several minutes
# to complete.
sudo apt-get remove --purge -y man-db

# Install clang-15, lld-15, and debootstrap.
echo "deb http://apt.llvm.org/focal/ llvm-toolchain-focal-15 main" |
  sudo dd of=/etc/apt/sources.list.d/llvm-toolchain-focal-15.list
curl https://apt.llvm.org/llvm-snapshot.gpg.key |
  gpg --dearmor                                 |
sudo dd of=/etc/apt/trusted.gpg.d/llvm-snapshot.gpg
sudo apt-get update
# this was unreliable sometimes, so try again if it fails
${installPkgsCommand} || echo 'Failed. Trying again.' && sudo apt-get clean && sudo apt-get update && ${installPkgsCommand}

# Create ubuntu-16.04 sysroot environment, which is used to avoid
# depending on a very recent version of glibc.
# \`libc6-dev\` is required for building any C source files.
# \`file\` and \`make\` are needed to build libffi-sys.
# \`curl\` is needed to build rusty_v8.
sudo debootstrap                                     \\
  --include=ca-certificates,curl,file,libc6-dev,make \\
  --no-merged-usr --variant=minbase xenial /sysroot  \\
  http://azure.archive.ubuntu.com/ubuntu
sudo mount --rbind /dev /sysroot/dev
sudo mount --rbind /sys /sysroot/sys
sudo mount --rbind /home /sysroot/home
sudo mount -t proc /proc /sysroot/proc

# Configure the build environment. Both Rust and Clang will produce
# llvm bitcode only, so we can use lld's incremental LTO support.
cat >> $GITHUB_ENV << __0
CARGO_PROFILE_BENCH_INCREMENTAL=false
CARGO_PROFILE_BENCH_LTO=false
CARGO_PROFILE_RELEASE_INCREMENTAL=false
CARGO_PROFILE_RELEASE_LTO=false
RUSTFLAGS<<__1
  -C linker-plugin-lto=true
  -C linker=clang-15
  -C link-arg=-fuse-ld=lld-15
  -C link-arg=--sysroot=/sysroot
  -C link-arg=-Wl,--allow-shlib-undefined
  -C link-arg=-Wl,--thinlto-cache-dir=$(pwd)/target/release/lto-cache
  -C link-arg=-Wl,--thinlto-cache-policy,cache_size_bytes=700m
  \${{ env.RUSTFLAGS }}
__1
RUSTDOCFLAGS<<__1
  -C linker-plugin-lto=true
  -C linker=clang-15
  -C link-arg=-fuse-ld=lld-15
  -C link-arg=--sysroot=/sysroot
  -C link-arg=-Wl,--allow-shlib-undefined
  -C link-arg=-Wl,--thinlto-cache-dir=$(pwd)/target/release/lto-cache
  -C link-arg=-Wl,--thinlto-cache-policy,cache_size_bytes=700m
  \${{ env.RUSTFLAGS }}
__1
CC=clang-15
CFLAGS=-flto=thin --sysroot=/sysroot
__0`,
};

const submoduleStep = (submodule: string) => ({
  name: `Clone submodule ${submodule}`,
  run: `git submodule update --init --recursive --depth=1 -- ${submodule}`,
});

const installRustStep = {
  uses: "dtolnay/rust-toolchain@stable",
};
const installPythonSteps = [{
  name: "Install Python",
  uses: "actions/setup-python@v4",
  with: { "python-version": 3.11 },
}, {
  name: "Remove unused versions of Python",
  if: "startsWith(matrix.os, 'windows')",
  shell: "pwsh",
  run: [
    '$env:PATH -split ";" |',
    '  Where-Object { Test-Path "$_\\python.exe" } |',
    "  Select-Object -Skip 1 |",
    '  ForEach-Object { Move-Item "$_" "$_.disabled" }',
  ].join("\n"),
}];
const installNodeStep = {
  name: "Install Node",
  uses: "actions/setup-node@v3",
  with: { "node-version": 18 },
};
const installDenoStep = {
  name: "Install Deno",
  uses: "denoland/setup-deno@v1",
  with: { "deno-version": "v1.x" },
};

const authenticateWithGoogleCloud = {
  name: "Authenticate with Google Cloud",
  uses: "google-github-actions/auth@v1",
  with: {
    "project_id": "denoland",
    "credentials_json": "${{ secrets.GCP_SA_KEY }}",
    "export_environment_variables": true,
    "create_credentials_file": true,
  },
};

function cancelEarlyIfDraftPr(
  nextSteps: Record<string, unknown>[],
): Record<string, unknown>[] {
  // Couple issues with GH Actions:
  //
  // 1. The pull_request event type does not include the commit message, so
  //    there's no way to override this with a commit message without running
  //    the workflow.
  // 2. Currently no way to early exit in GH Actions, so we need to do all these
  //    if conditions. Waiting on: https://github.com/actions/runner/issues/662
  //
  // The solution below will only occur on draft PRs and only run the CI if the
  // commit message contains [ci].
  return [
    {
      name: "Cancel if draft PR",
      id: "exit_early",
      if: "github.event.pull_request.draft == true",
      shell: "bash",
      run: [
        "GIT_MESSAGE=$(git log --format=%s -n 1 ${{github.event.after}})",
        "echo Commit message: $GIT_MESSAGE",
        "echo $GIT_MESSAGE | grep '\\[ci\\]' || (echo 'Exiting due to draft PR. Commit with [ci] to bypass.' ; echo 'EXIT_EARLY=true' >> $GITHUB_OUTPUT)",
      ].join("\n"),
    },
    ...nextSteps.map((step) =>
      withCondition(step, "steps.exit_early.outputs.EXIT_EARLY != 'true'")
    ),
  ];
}

function skipJobsIfPrAndMarkedSkip(
  steps: Record<string, unknown>[],
): Record<string, unknown>[] {
  // GitHub does not make skipping a specific matrix element easy
  // so just apply this condition to all the steps.
  // https://stackoverflow.com/questions/65384420/how-to-make-a-github-action-matrix-element-conditional
  return steps.map((s) =>
    withCondition(
      s,
      "!(github.event_name == 'pull_request' && matrix.skip_pr)",
    )
  );
}

function withCondition(
  step: Record<string, unknown>,
  condition: string,
): Record<string, unknown> {
  return {
    ...step,
    if: "if" in step ? `${condition} && (${step.if})` : condition,
  };
}

const ci = {
  name: "ci",
  on: {
    push: {
      branches: ["main"],
      tags: ["*"],
    },
    pull_request: {
      types: [
        "opened",
        "reopened",
        "synchronize",
        // need to re-run the action when converting from draft because
        // draft PRs will not necessarily run all the steps
        "ready_for_review",
      ],
    },
  },
  concurrency: {
    group:
      "${{ github.workflow }}-${{ !contains(github.event.pull_request.labels.*.name, 'ci-test-flaky') && github.head_ref || github.run_id }}",
    "cancel-in-progress": true,
  },
  jobs: {
    build: {
      name: "${{ matrix.job }} ${{ matrix.profile }} ${{ matrix.os }}",
      "runs-on": "${{ matrix.runner || matrix.os }}",
      "timeout-minutes": 120,
      strategy: {
        matrix: {
          include: [
            {
              os: Runners.macos,
              job: "test",
              profile: "fastci",
            },
            {
              os: Runners.macos,
              job: "test",
              profile: "release",
              skip_pr: true,
            },
            {
              os: Runners.windows,
              job: "test",
              profile: "fastci",
            },
            {
              os: Runners.windows,
              // use a free runner on PRs since this will be skipped
              runner:
                `\${{ github.event_name == 'pull_request' && 'windows-2022' || (${windowsRunnerCondition}) }}`,
              job: "test",
              profile: "release",
              skip_pr: true,
            },
            {
              os: Runners.linux,
              job: "test",
              profile: "release",
              use_sysroot: true,
              // TODO(ry): Because CI is so slow on for OSX and Windows, we
              // currently run the Web Platform tests only on Linux.
              wpt: "${{ !startsWith(github.ref, 'refs/tags/') }}",
            },
            {
              os: Runners.linux,
              job: "bench",
              profile: "release",
              use_sysroot: true,
              skip_pr:
                "${{ !contains(github.event.pull_request.labels.*.name, 'ci-bench') }}",
            },
            {
              os: Runners.linux,
              job: "test",
              profile: "debug",
              use_sysroot: true,
              wpt:
                "${{ github.ref == 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/') }}",
            },
            {
              os: Runners.linux,
              job: "lint",
              profile: "debug",
            },
          ],
        },
        // Always run main branch builds to completion. This allows the cache to
        // stay mostly up-to-date in situations where a single job fails due to
        // e.g. a flaky test.
        // Don't fast-fail on tag build because publishing binaries shouldn't be
        // prevented if any of the stages fail (which can be a false negative).
        "fail-fast":
          "${{ github.event_name == 'pull_request' || (github.ref != 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')) }}",
      },
      env: {
        CARGO_TERM_COLOR: "always",
        RUST_BACKTRACE: "full",
      },
      steps: skipJobsIfPrAndMarkedSkip([
        {
          name: "Configure git",
          run: [
            "git config --global core.symlinks true",
            "git config --global fetch.parallel 32",
          ].join("\n"),
        },
        {
          name: "Clone repository",
          uses: "actions/checkout@v3",
          with: {
            // Use depth > 1, because sometimes we need to rebuild main and if
            // other commits have landed it will become impossible to rebuild if
            // the checkout is too shallow.
            "fetch-depth": 5,
            submodules: false,
          },
        },
        ...cancelEarlyIfDraftPr([
          submoduleStep("./test_util/std"),
          {
            ...submoduleStep("./test_util/wpt"),
            if: "matrix.wpt",
          },
          {
            ...submoduleStep("./third_party"),
            if: "matrix.job == 'lint' || matrix.job == 'bench'",
          },
          {
            name: "Create source tarballs (release, linux)",
            if: [
              "startsWith(matrix.os, 'ubuntu') &&",
              "matrix.profile == 'release' &&",
              "matrix.job == 'test' &&",
              "github.repository == 'denoland/deno' &&",
              "startsWith(github.ref, 'refs/tags/')",
            ].join("\n"),
            run: [
              "mkdir -p target/release",
              'tar --exclude=".git*" --exclude=target --exclude=third_party/prebuilt \\',
              "    -czvf target/release/deno_src.tar.gz -C .. deno",
            ].join("\n"),
          },
          installRustStep,
          {
            if: "matrix.job == 'lint' || matrix.job == 'test'",
            ...installDenoStep,
          },
          ...installPythonSteps.map((s) =>
            withCondition(s, "matrix.job != 'lint'")
          ),
          {
            // only necessary for benchmarks
            if: "matrix.job == 'bench'",
            ...installNodeStep,
          },
          {
            if: [
              "matrix.profile == 'release' &&",
              "matrix.job == 'test' &&",
              "github.repository == 'denoland/deno' &&",
              "(github.ref == 'refs/heads/main' ||",
              "startsWith(github.ref, 'refs/tags/'))",
            ].join("\n"),
            ...authenticateWithGoogleCloud,
          },
          {
            name: "Setup gcloud (unix)",
            if: [
              "runner.os != 'Windows' &&",
              "matrix.profile == 'release' &&",
              "matrix.job == 'test' &&",
              "github.repository == 'denoland/deno' &&",
              "(github.ref == 'refs/heads/main' ||",
              "startsWith(github.ref, 'refs/tags/'))",
            ].join("\n"),
            uses: "google-github-actions/setup-gcloud@v1",
            with: {
              project_id: "denoland",
            },
          },
          {
            name: "Setup gcloud (windows)",
            if: [
              "runner.os == 'Windows' &&",
              "matrix.profile == 'release' &&",
              "matrix.job == 'test' &&",
              "github.repository == 'denoland/deno' &&",
              "(github.ref == 'refs/heads/main' ||",
              "startsWith(github.ref, 'refs/tags/'))",
            ].join("\n"),
            uses: "google-github-actions/setup-gcloud@v1",
            env: {
              CLOUDSDK_PYTHON: "${{env.pythonLocation}}\\python.exe",
            },
            with: {
              project_id: "denoland",
            },
          },
          {
            name: "Configure canary build",
            if: [
              "matrix.job == 'test' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno' &&",
              "github.ref == 'refs/heads/main'",
            ].join("\n"),
            shell: "bash",
            run: 'echo "DENO_CANARY=true" >> $GITHUB_ENV',
          },
          {
            if: "matrix.use_sysroot",
            ...sysRootStep,
          },
          {
            name: "Log versions",
            shell: "bash",
            run: [
              "python --version",
              "rustc --version",
              "cargo --version",
              // Deno is installed when linting.
              'if [ "${{ matrix.job }}" == "lint" ]',
              "then",
              "  deno --version",
              "fi",
              // Node is installed for benchmarks.
              'if [ "${{ matrix.job }}" == "bench" ]',
              "then",
              "  node -v",
              "fi",
            ].join("\n"),
          },
          {
            name: "Cache Cargo home",
            uses: "actions/cache@v3",
            with: {
              // See https://doc.rust-lang.org/cargo/guide/cargo-home.html#caching-the-cargo-home-in-ci
              path: [
                "~/.cargo/registry/index",
                "~/.cargo/registry/cache",
                "~/.cargo/git/db",
              ].join("\n"),
              key:
                "18-cargo-home-${{ matrix.os }}-${{ hashFiles('Cargo.lock') }}",
            },
          },
          {
            // Restore cache from the latest 'main' branch build.
            name: "Restore cache build output (PR)",
            uses: "actions/cache/restore@v3",
            if:
              "github.ref != 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')",
            with: {
              path: [
                "./target",
                "!./target/*/gn_out",
                "!./target/*/*.zip",
                "!./target/*/*.tar.gz",
              ].join("\n"),
              key: "never_saved",
              "restore-keys":
                "19-cargo-target-${{ matrix.os }}-${{ matrix.profile }}-",
            },
          },
          {
            name: "Apply and update mtime cache",
            if: "!startsWith(github.ref, 'refs/tags/')",
            uses: "./.github/mtime_cache",
            with: {
              "cache-path": "./target",
            },
          },
          {
            // Shallow the cloning the crates.io index makes CI faster because it
            // obviates the need for Cargo to clone the index. If we don't do this
            // Cargo will `git clone` the github repository that contains the entire
            // history of the crates.io index from github. We don't believe the
            // identifier '1ecc6299db9ec823' will ever change, but if it does then this
            // command must be updated.
            name: "Shallow clone crates.io index",
            shell: "bash",
            run: [
              "if [ ! -d ~/.cargo/registry/index/github.com-1ecc6299db9ec823/.git ]",
              "then",
              "  git clone --depth 1 --no-checkout                      \\",
              "            https://github.com/rust-lang/crates.io-index \\",
              "            ~/.cargo/registry/index/github.com-1ecc6299db9ec823",
              "fi",
            ].join("\n"),
          },
          {
            name: "test_format.js",
            if: "matrix.job == 'lint'",
            run:
              "deno run --unstable --allow-write --allow-read --allow-run ./tools/format.js --check",
          },
          {
            name: "lint.js",
            if: "matrix.job == 'lint'",
            run:
              "deno run --unstable --allow-write --allow-read --allow-run ./tools/lint.js",
          },
          {
            name: "Build debug",
            if: [
              "(matrix.job == 'test' || matrix.job == 'bench') &&",
              "matrix.profile == 'debug'",
            ].join("\n"),
            run: "cargo build --locked --all-targets",
          },
          {
            name: "Build fastci",
            if: "(matrix.job == 'test' && matrix.profile == 'fastci')",
            run: "cargo build --locked --all-targets",
            env: { CARGO_PROFILE_DEV_DEBUG: 0 },
          },
          {
            name: "Build release",
            if: [
              "(matrix.job == 'test' || matrix.job == 'bench') &&",
              "matrix.profile == 'release' && (matrix.use_sysroot ||",
              "(github.repository == 'denoland/deno' &&",
              "(github.ref == 'refs/heads/main' ||",
              "startsWith(github.ref, 'refs/tags/'))))",
            ].join("\n"),
            run: "cargo build --release --locked --all-targets",
          },
          {
            name: "Upload PR artifact (linux)",
            if: [
              "matrix.job == 'test' &&",
              "matrix.profile == 'release' && (matrix.use_sysroot ||",
              "(github.repository == 'denoland/deno' &&",
              "(github.ref == 'refs/heads/main' ||",
              "startsWith(github.ref, 'refs/tags/'))))",
            ].join("\n"),
            uses: "actions/upload-artifact@v3",
            with: {
              name: "deno-${{ github.event.number }}",
              path: "target/release/deno",
            },
          },
          {
            name: "Pre-release (linux)",
            if: [
              "startsWith(matrix.os, 'ubuntu') &&",
              "matrix.job == 'test' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno'",
            ].join("\n"),
            run: [
              "cd target/release",
              "zip -r deno-x86_64-unknown-linux-gnu.zip deno",
              "./deno types > lib.deno.d.ts",
            ].join("\n"),
          },
          {
            name: "Pre-release (mac)",
            if: [
              "startsWith(matrix.os, 'macOS') &&",
              "matrix.job == 'test' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno' &&",
              "(github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))",
            ].join("\n"),
            run: [
              "cd target/release",
              "zip -r deno-x86_64-apple-darwin.zip deno",
            ]
              .join("\n"),
          },
          {
            name: "Pre-release (windows)",
            if: [
              "startsWith(matrix.os, 'windows') &&",
              "matrix.job == 'test' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno' &&",
              "(github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))",
            ].join("\n"),
            run:
              "Compress-Archive -CompressionLevel Optimal -Force -Path target/release/deno.exe -DestinationPath target/release/deno-x86_64-pc-windows-msvc.zip",
          },
          {
            name: "Upload canary to dl.deno.land (unix)",
            if: [
              "runner.os != 'Windows' &&",
              "matrix.job == 'test' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno' &&",
              "github.ref == 'refs/heads/main'",
            ].join("\n"),
            run:
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/canary/$(git rev-parse HEAD)/',
          },
          {
            name: "Upload canary to dl.deno.land (windows)",
            if: [
              "runner.os == 'Windows' &&",
              "matrix.job == 'test' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno' &&",
              "github.ref == 'refs/heads/main'",
            ].join("\n"),
            env: {
              CLOUDSDK_PYTHON: "${{env.pythonLocation}}\\python.exe",
            },
            shell: "bash",
            run:
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/canary/$(git rev-parse HEAD)/',
          },
          {
            name: "Test debug",
            if: [
              "matrix.job == 'test' && matrix.profile == 'debug' &&",
              "!startsWith(github.ref, 'refs/tags/')",
            ].join("\n"),
            run: "cargo test --locked",
          },
          {
            name: "Test fastci",
            if: "matrix.job == 'test' && matrix.profile == 'fastci'",
            run: [
              // Run unit then integration tests. Skip doc tests here
              // since they are sometimes very slow on Mac.
              "cargo test --locked --lib",
              "cargo test --locked --test '*'",
            ].join("\n"),
            env: {
              CARGO_PROFILE_DEV_DEBUG: 0,
            },
          },
          {
            name: "Test release",
            if: [
              "matrix.job == 'test' && matrix.profile == 'release' &&",
              "(matrix.use_sysroot || (",
              "github.repository == 'denoland/deno' &&",
              "github.ref == 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')))",
            ].join("\n"),
            run: "cargo test --release --locked",
          },
          {
            // Since all tests are skipped when we're building a tagged commit
            // this is a minimal check to ensure that binary is not corrupted
            name: "Check deno binary",
            if:
              "matrix.profile == 'release' && startsWith(github.ref, 'refs/tags/')",
            shell: "bash",
            run: 'target/release/deno eval "console.log(1+2)" | grep 3',
            env: {
              NO_COLOR: 1,
            },
          },
          {
            // Verify that the binary actually works in the Ubuntu-16.04 sysroot.
            name: "Check deno binary (in sysroot)",
            if: "matrix.profile == 'release' && matrix.use_sysroot",
            run: 'sudo chroot /sysroot "$(pwd)/target/release/deno" --version',
          },
          {
            name: "Configure hosts file for WPT",
            if: "matrix.wpt",
            run: "./wpt make-hosts-file | sudo tee -a /etc/hosts",
            "working-directory": "test_util/wpt/",
          },
          {
            name: "Run web platform tests (debug)",
            if: "matrix.wpt && matrix.profile == 'debug'",
            env: {
              DENO_BIN: "./target/debug/deno",
            },
            run: [
              "deno run --allow-env --allow-net --allow-read --allow-run \\",
              "        --allow-write --unstable                         \\",
              "        --lock=tools/deno.lock.json                      \\",
              "        ./tools/wpt.ts setup",
              "deno run --allow-env --allow-net --allow-read --allow-run \\",
              "         --allow-write --unstable                         \\",
              "         --lock=tools/deno.lock.json              \\",
              '         ./tools/wpt.ts run --quiet --binary="$DENO_BIN"',
            ].join("\n"),
          },
          {
            name: "Run web platform tests (release)",
            if: "matrix.wpt && matrix.profile == 'release'",
            env: {
              DENO_BIN: "./target/release/deno",
            },
            run: [
              "deno run --allow-env --allow-net --allow-read --allow-run \\",
              "         --allow-write --unstable                         \\",
              "         --lock=tools/deno.lock.json                      \\",
              "         ./tools/wpt.ts setup",
              "deno run --allow-env --allow-net --allow-read --allow-run \\",
              "         --allow-write --unstable                         \\",
              "         --lock=tools/deno.lock.json                      \\",
              "         ./tools/wpt.ts run --quiet --release             \\",
              '                            --binary="$DENO_BIN"          \\',
              "                            --json=wpt.json               \\",
              "                            --wptreport=wptreport.json",
            ].join("\n"),
          },
          {
            name: "Upload wpt results to dl.deno.land",
            "continue-on-error": true,
            if: [
              "matrix.wpt &&",
              "runner.os == 'Linux' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno' &&",
              "github.ref == 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')",
            ].join("\n"),
            run: [
              "gzip ./wptreport.json",
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./wpt.json gs://dl.deno.land/wpt/$(git rev-parse HEAD).json',
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./wptreport.json.gz gs://dl.deno.land/wpt/$(git rev-parse HEAD)-wptreport.json.gz',
              "echo $(git rev-parse HEAD) > wpt-latest.txt",
              'gsutil -h "Cache-Control: no-cache" cp wpt-latest.txt gs://dl.deno.land/wpt-latest.txt',
            ].join("\n"),
          },
          {
            name: "Upload wpt results to wpt.fyi",
            "continue-on-error": true,
            if: [
              "matrix.wpt &&",
              "runner.os == 'Linux' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno' &&",
              "github.ref == 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')",
            ].join("\n"),
            env: {
              WPT_FYI_USER: "deno",
              WPT_FYI_PW: "${{ secrets.WPT_FYI_PW }}",
              GITHUB_TOKEN: "${{ secrets.DENOBOT_PAT }}",
            },
            run: [
              "./target/release/deno run --allow-all --lock=tools/deno.lock.json \\",
              "    ./tools/upload_wptfyi.js $(git rev-parse HEAD) --ghstatus",
            ].join("\n"),
          },
          {
            name: "Run benchmarks",
            if:
              "matrix.job == 'bench' && !startsWith(github.ref, 'refs/tags/')",
            run: "cargo bench --locked",
          },
          {
            name: "Post Benchmarks",
            if: [
              "matrix.job == 'bench' &&",
              "github.repository == 'denoland/deno' &&",
              "github.ref == 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')",
            ].join("\n"),
            env: {
              DENOBOT_PAT: "${{ secrets.DENOBOT_PAT }}",
            },
            run: [
              "git clone --depth 1 --branch gh-pages                             \\",
              "    https://${DENOBOT_PAT}@github.com/denoland/benchmark_data.git \\",
              "    gh-pages",
              "./target/release/deno run --allow-all --unstable \\",
              "    ./tools/build_benchmark_jsons.js --release",
              "cd gh-pages",
              'git config user.email "propelml@gmail.com"',
              'git config user.name "denobot"',
              "git add .",
              'git commit --message "Update benchmarks"',
              "git push origin gh-pages",
            ].join("\n"),
          },
          {
            name: "Build product size info",
            if:
              "matrix.job != 'lint' && matrix.profile != 'fastci' && github.repository == 'denoland/deno' && (github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))",
            run: [
              'du -hd1 "./target/${{ matrix.profile }}"',
              'du -ha  "./target/${{ matrix.profile }}/deno"',
            ].join("\n"),
          },
          {
            name: "Worker info",
            if: "matrix.job == 'bench'",
            run: [
              "cat /proc/cpuinfo",
              "cat /proc/meminfo",
            ].join("\n"),
          },
          {
            name: "Upload release to dl.deno.land (unix)",
            if: [
              "runner.os != 'Windows' &&",
              "matrix.job == 'test' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno' &&",
              "startsWith(github.ref, 'refs/tags/')",
            ].join("\n"),
            run:
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
          },
          {
            name: "Upload release to dl.deno.land (windows)",
            if: [
              "runner.os == 'Windows' &&",
              "matrix.job == 'test' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno' &&",
              "startsWith(github.ref, 'refs/tags/')",
            ].join("\n"),
            env: {
              CLOUDSDK_PYTHON: "${{env.pythonLocation}}\\python.exe",
            },
            shell: "bash",
            run:
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
          },
          {
            name: "Create release notes",
            shell: "bash",
            if: [
              "matrix.job == 'test' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno' &&",
              "startsWith(github.ref, 'refs/tags/')",
            ].join("\n"),
            run: [
              "export PATH=$PATH:$(pwd)/target/release",
              "./tools/release/05_create_release_notes.ts",
            ].join("\n"),
          },
          {
            name: "Upload release to GitHub",
            uses: "softprops/action-gh-release@v0.1.15",
            if: [
              "matrix.job == 'test' &&",
              "matrix.profile == 'release' &&",
              "github.repository == 'denoland/deno' &&",
              "startsWith(github.ref, 'refs/tags/')",
            ].join("\n"),
            env: {
              GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}",
            },
            with: {
              files: [
                "target/release/deno-x86_64-pc-windows-msvc.zip",
                "target/release/deno-x86_64-unknown-linux-gnu.zip",
                "target/release/deno-x86_64-apple-darwin.zip",
                "target/release/deno_src.tar.gz",
                "target/release/lib.deno.d.ts",
              ].join("\n"),
              body_path: "target/release/release-notes.md",
              draft: true,
            },
          },
          {
            // In main branch, always creates fresh cache
            name: "Save cache build output (main)",
            uses: "actions/cache/save@v3",
            if:
              "(matrix.profile == 'release' || matrix.profile == 'fastci') && github.ref == 'refs/heads/main'",
            with: {
              path: [
                "./target",
                "!./target/*/gn_out",
                "!./target/*/*.zip",
                "!./target/*/*.tar.gz",
              ].join("\n"),
              key:
                "18-cargo-target-${{ matrix.os }}-${{ matrix.profile }}-${{ github.sha }}",
            },
          },
        ]),
      ]),
    },
    "publish-canary": {
      name: "publish canary",
      "runs-on": "ubuntu-20.04",
      needs: ["build"],
      if:
        "github.repository == 'denoland/deno' && github.ref == 'refs/heads/main'",
      steps: [
        authenticateWithGoogleCloud,
        {
          name: "Setup gcloud",
          uses: "google-github-actions/setup-gcloud@v1",
          with: {
            project_id: "denoland",
          },
        },
        {
          name: "Upload canary version file to dl.deno.land",
          run: [
            "echo ${{ github.sha }} > canary-latest.txt",
            'gsutil -h "Cache-Control: no-cache" cp canary-latest.txt gs://dl.deno.land/canary-latest.txt',
          ].join("\n"),
        },
      ],
    },
  },
};

let finalText = `# GENERATED BY ./ci.generate.ts -- DO NOT DIRECTLY EDIT\n\n`;
finalText += yaml.stringify(ci, {
  noRefs: true,
  lineWidth: 10_000,
  noCompatMode: true,
});

Deno.writeTextFileSync(new URL("./ci.yml", import.meta.url), finalText);
