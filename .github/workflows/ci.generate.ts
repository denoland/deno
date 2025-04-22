#!/usr/bin/env -S deno run --allow-write=. --lock=./tools/deno.lock.json
// Copyright 2018-2025 the Deno authors. MIT license.
import { stringify } from "jsr:@std/yaml@^0.221/stringify";

// Bump this number when you want to purge the cache.
// Note: the tools/release/01_bump_crate_versions.ts script will update this version
// automatically via regex, so ensure that this line maintains this format.
const cacheVersion = 53;

const ubuntuX86Runner = "ubuntu-24.04";
const ubuntuX86XlRunner = "ubuntu-24.04-xl";
const ubuntuARMRunner = "ubicloud-standard-16-arm";
const windowsX86Runner = "windows-2022";
const windowsX86XlRunner = "windows-2022-xl";
const macosX86Runner = "macos-13";
const macosArmRunner = "macos-14";
const selfHostedMacosArmRunner = "ghcr.io/cirruslabs/macos-runner:sonoma";

const Runners = {
  linuxX86: {
    os: "linux",
    arch: "x86_64",
    runner: ubuntuX86Runner,
  },
  linuxX86Xl: {
    os: "linux",
    arch: "x86_64",
    runner:
      `\${{ github.repository == 'denoland/deno' && '${ubuntuX86XlRunner}' || '${ubuntuX86Runner}' }}`,
  },
  linuxArm: {
    os: "linux",
    arch: "aarch64",
    runner: ubuntuARMRunner,
  },
  macosX86: {
    os: "macos",
    arch: "x86_64",
    runner: macosX86Runner,
  },
  macosArm: {
    os: "macos",
    arch: "aarch64",
    runner: macosArmRunner,
  },
  macosArmSelfHosted: {
    os: "macos",
    arch: "aarch64",
    // Actually use self-hosted runner only in denoland/deno on `main` branch and for tags (release) builds.
    runner:
      `\${{ github.repository == 'denoland/deno' && (github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/')) && '${selfHostedMacosArmRunner}' || '${macosArmRunner}' }}`,
  },
  windowsX86: {
    os: "windows",
    arch: "x86_64",
    runner: windowsX86Runner,
  },
  windowsX86Xl: {
    os: "windows",
    arch: "x86_64",
    runner:
      `\${{ github.repository == 'denoland/deno' && '${windowsX86XlRunner}' || '${windowsX86Runner}' }}`,
  },
} as const;

const prCacheKeyPrefix =
  `${cacheVersion}-cargo-target-\${{ matrix.os }}-\${{ matrix.arch }}-\${{ matrix.profile }}-\${{ matrix.job }}-`;
const prCacheKey = `${prCacheKeyPrefix}\${{ github.sha }}`;
const prCachePath = [
  // this must match for save and restore (https://github.com/actions/cache/issues/1444)
  "./target",
  "!./target/*/gn_out",
  "!./target/*/gn_root",
  "!./target/*/*.zip",
  "!./target/*/*.tar.gz",
].join("\n");

// Note that you may need to add more version to the `apt-get remove` line below if you change this
const llvmVersion = 19;
const installPkgsCommand =
  `sudo apt-get install --no-install-recommends clang-${llvmVersion} lld-${llvmVersion} clang-tools-${llvmVersion} clang-format-${llvmVersion} clang-tidy-${llvmVersion}`;
const sysRootStep = {
  name: "Set up incremental LTO and sysroot build",
  run: `# Setting up sysroot
export DEBIAN_FRONTEND=noninteractive
# Avoid running man-db triggers, which sometimes takes several minutes
# to complete.
sudo apt-get -qq remove --purge -y man-db  > /dev/null 2> /dev/null
# Remove older clang before we install
sudo apt-get -qq remove \
  'clang-12*' 'clang-13*' 'clang-14*' 'clang-15*' 'clang-16*' 'clang-17*' 'clang-18*' 'llvm-12*' 'llvm-13*' 'llvm-14*' 'llvm-15*' 'llvm-16*' 'lld-12*' 'lld-13*' 'lld-14*' 'lld-15*' 'lld-16*' 'lld-17*' 'lld-18*' > /dev/null 2> /dev/null

# Install clang-XXX, lld-XXX, and debootstrap.
echo "deb http://apt.llvm.org/jammy/ llvm-toolchain-jammy-${llvmVersion} main" |
  sudo dd of=/etc/apt/sources.list.d/llvm-toolchain-jammy-${llvmVersion}.list
curl https://apt.llvm.org/llvm-snapshot.gpg.key |
  gpg --dearmor                                 |
sudo dd of=/etc/apt/trusted.gpg.d/llvm-snapshot.gpg
sudo apt-get update
# this was unreliable sometimes, so try again if it fails
${installPkgsCommand} || echo 'Failed. Trying again.' && sudo apt-get clean && sudo apt-get update && ${installPkgsCommand}
# Fix alternatives
(yes '' | sudo update-alternatives --force --all) > /dev/null 2> /dev/null || true

clang-${llvmVersion} -c -o /tmp/memfd_create_shim.o tools/memfd_create_shim.c -fPIC

echo "Decompressing sysroot..."
wget -q https://github.com/denoland/deno_sysroot_build/releases/download/sysroot-20241030/sysroot-\`uname -m\`.tar.xz -O /tmp/sysroot.tar.xz
cd /
xzcat /tmp/sysroot.tar.xz | sudo tar -x
sudo mount --rbind /dev /sysroot/dev
sudo mount --rbind /sys /sysroot/sys
sudo mount --rbind /home /sysroot/home
sudo mount -t proc /proc /sysroot/proc
cd

echo "Done."

# Configure the build environment. Both Rust and Clang will produce
# llvm bitcode only, so we can use lld's incremental LTO support.

# Load the sysroot's env vars
echo "sysroot env:"
cat /sysroot/.env
. /sysroot/.env

# Important notes:
#   1. -ldl seems to be required to avoid a failure in FFI tests. This flag seems
#      to be in the Rust default flags in the smoketest, so uncertain why we need
#      to be explicit here.
#   2. RUSTFLAGS and RUSTDOCFLAGS must be specified, otherwise the doctests fail
#      to build because the object formats are not compatible.
echo "
CARGO_PROFILE_BENCH_INCREMENTAL=false
CARGO_PROFILE_RELEASE_INCREMENTAL=false
RUSTFLAGS<<__1
  -C linker-plugin-lto=true
  -C linker=clang-${llvmVersion}
  -C link-arg=-fuse-ld=lld-${llvmVersion}
  -C link-arg=-ldl
  -C link-arg=-Wl,--allow-shlib-undefined
  -C link-arg=-Wl,--thinlto-cache-dir=$(pwd)/target/release/lto-cache
  -C link-arg=-Wl,--thinlto-cache-policy,cache_size_bytes=700m
  -C link-arg=/tmp/memfd_create_shim.o
  --cfg tokio_unstable
  $RUSTFLAGS
__1
RUSTDOCFLAGS<<__1
  -C linker-plugin-lto=true
  -C linker=clang-${llvmVersion}
  -C link-arg=-fuse-ld=lld-${llvmVersion}
  -C link-arg=-ldl
  -C link-arg=-Wl,--allow-shlib-undefined
  -C link-arg=-Wl,--thinlto-cache-dir=$(pwd)/target/release/lto-cache
  -C link-arg=-Wl,--thinlto-cache-policy,cache_size_bytes=700m
  -C link-arg=/tmp/memfd_create_shim.o
  --cfg tokio_unstable
  $RUSTFLAGS
__1
CC=/usr/bin/clang-${llvmVersion}
CFLAGS=$CFLAGS
" > $GITHUB_ENV`,
};

const installBenchTools = "./tools/install_prebuilt.js wrk hyperfine";

const cloneRepoStep = [{
  name: "Configure git",
  run: [
    "git config --global core.symlinks true",
    "git config --global fetch.parallel 32",
  ].join("\n"),
}, {
  name: "Clone repository",
  uses: "actions/checkout@v4",
  with: {
    // Use depth > 1, because sometimes we need to rebuild main and if
    // other commits have landed it will become impossible to rebuild if
    // the checkout is too shallow.
    "fetch-depth": 5,
    submodules: false,
  },
}];

const submoduleStep = (submodule: string) => ({
  name: `Clone submodule ${submodule}`,
  run: `git submodule update --init --recursive --depth=1 -- ${submodule}`,
});

const installRustStep = {
  uses: "dsherret/rust-toolchain-file@v1",
};
const installPythonSteps = [{
  name: "Install Python",
  uses: "actions/setup-python@v5",
  with: { "python-version": 3.11 },
}, {
  name: "Remove unused versions of Python",
  if: "matrix.os == 'windows'",
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
  uses: "actions/setup-node@v4",
  with: { "node-version": 18 },
};
const installDenoStep = {
  name: "Install Deno",
  uses: "denoland/setup-deno@v2",
  with: { "deno-version": "v2.x" },
};

const authenticateWithGoogleCloud = {
  name: "Authenticate with Google Cloud",
  uses: "google-github-actions/auth@v2",
  with: {
    "project_id": "denoland",
    "credentials_json": "${{ secrets.GCP_SA_KEY }}",
    "export_environment_variables": true,
    "create_credentials_file": true,
  },
};

function skipJobsIfPrAndMarkedSkip(
  steps: Record<string, unknown>[],
): Record<string, unknown>[] {
  // GitHub does not make skipping a specific matrix element easy
  // so just apply this condition to all the steps.
  // https://stackoverflow.com/questions/65384420/how-to-make-a-github-action-matrix-element-conditional
  return steps.map((s) =>
    withCondition(
      s,
      "!(matrix.skip)",
    )
  );
}

function onlyIfDraftPr(
  steps: Record<string, unknown>[],
): Record<string, unknown>[] {
  return steps.map((s) =>
    withCondition(
      s,
      "github.event.pull_request.draft == true",
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

function removeSurroundingExpression(text: string) {
  if (text.startsWith("${{")) {
    return text.replace(/^\${{/, "").replace(/}}$/, "").trim();
  } else {
    return `'${text}'`;
  }
}

function handleMatrixItems(items: {
  skip_pr?: string | true;
  skip?: string;
  os: "linux" | "macos" | "windows";
  arch: "x86_64" | "aarch64";
  runner: string;
  profile?: string;
  job?: string;
  use_sysroot?: boolean;
  wpt?: string;
}[]) {
  return items.map((item) => {
    // use a free "ubuntu" runner on jobs that are skipped

    // skip_pr is shorthand for skip = github.event_name == 'pull_request'.
    if (item.skip_pr != null) {
      if (item.skip_pr === true) {
        item.skip = "${{ github.event_name == 'pull_request' }}";
      } else if (typeof item.skip_pr === "string") {
        item.skip = "${{ github.event_name == 'pull_request' && " +
          removeSurroundingExpression(item.skip_pr.toString()) + " }}";
      }
      delete item.skip_pr;
    }

    if (typeof item.skip === "string") {
      let runner =
        "${{ (!contains(github.event.pull_request.labels.*.name, 'ci-full') && (";
      runner += removeSurroundingExpression(item.skip.toString()) + ")) && ";
      runner += `'${ubuntuX86Runner}' || ${
        removeSurroundingExpression(item.runner)
      } }}`;

      // deno-lint-ignore no-explicit-any
      (item as any).runner = runner;
      item.skip =
        "${{ !contains(github.event.pull_request.labels.*.name, 'ci-full') && (" +
        removeSurroundingExpression(item.skip.toString()) + ") }}";
    }

    return { ...item };
  });
}

const ci = {
  name: "ci",
  permissions: {
    contents: "write",
  },
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
    // The pre_build step is used to skip running the CI on draft PRs and to not even
    // start the build job. This can be overridden by adding [ci] to the commit title
    pre_build: {
      name: "pre-build",
      "runs-on": "ubuntu-latest",
      outputs: {
        skip_build: "${{ steps.check.outputs.skip_build }}",
      },
      steps: onlyIfDraftPr([
        ...cloneRepoStep,
        {
          id: "check",
          if: "!contains(github.event.pull_request.labels.*.name, 'ci-draft')",
          run: [
            "GIT_MESSAGE=$(git log --format=%s -n 1 ${{github.event.after}})",
            "echo Commit message: $GIT_MESSAGE",
            "echo $GIT_MESSAGE | grep '\\[ci\\]' || (echo 'Exiting due to draft PR. Commit with [ci] to bypass or add the ci-draft label.' ; echo 'skip_build=true' >> $GITHUB_OUTPUT)",
          ].join("\n"),
        },
      ]),
    },
    build: {
      name:
        "${{ matrix.job }} ${{ matrix.profile }} ${{ matrix.os }}-${{ matrix.arch }}",
      needs: ["pre_build"],
      if: "${{ needs.pre_build.outputs.skip_build != 'true' }}",
      "runs-on": "${{ matrix.runner }}",
      "timeout-minutes": 240,
      defaults: {
        run: {
          // GH actions does not fail fast by default on
          // Windows, so we set bash as the default shell
          shell: "bash",
        },
      },
      strategy: {
        matrix: {
          include: handleMatrixItems([{
            ...Runners.macosX86,
            job: "test",
            profile: "debug",
          }, {
            ...Runners.macosX86,
            job: "test",
            profile: "release",
            skip_pr: true,
          }, {
            ...Runners.macosArm,
            job: "test",
            profile: "debug",
          }, {
            ...Runners.macosArmSelfHosted,
            job: "test",
            profile: "release",
            skip_pr: true,
          }, {
            ...Runners.windowsX86,
            job: "test",
            profile: "debug",
          }, {
            ...Runners.windowsX86Xl,
            job: "test",
            profile: "release",
            skip_pr: true,
          }, {
            ...Runners.linuxX86Xl,
            job: "test",
            profile: "release",
            use_sysroot: true,
            // TODO(ry): Because CI is so slow on for OSX and Windows, we
            // currently run the Web Platform tests only on Linux.
            wpt: "${{ !startsWith(github.ref, 'refs/tags/') }}",
          }, {
            ...Runners.linuxX86Xl,
            job: "bench",
            profile: "release",
            use_sysroot: true,
            skip_pr:
              "${{ !contains(github.event.pull_request.labels.*.name, 'ci-bench') }}",
          }, {
            ...Runners.linuxX86,
            job: "test",
            profile: "debug",
            use_sysroot: true,
          }, {
            ...Runners.linuxX86,
            job: "lint",
            profile: "debug",
          }, {
            ...Runners.linuxArm,
            job: "test",
            profile: "debug",
          }, {
            ...Runners.linuxArm,
            job: "test",
            profile: "release",
            use_sysroot: true,
          }, {
            ...Runners.macosX86,
            job: "lint",
            profile: "debug",
          }, {
            ...Runners.windowsX86,
            job: "lint",
            profile: "debug",
          }]),
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
        // disable anyhow's library backtrace
        RUST_LIB_BACKTRACE: 0,
      },
      steps: skipJobsIfPrAndMarkedSkip([
        ...cloneRepoStep,
        submoduleStep("./tests/util/std"),
        {
          ...submoduleStep("./tests/wpt/suite"),
          if: "matrix.wpt",
        },
        {
          ...submoduleStep("./tests/node_compat/runner/suite"),
          if: "matrix.job == 'lint' && matrix.os == 'linux'",
        },
        {
          ...submoduleStep("./cli/bench/testdata/lsp_benchdata"),
          if: "matrix.job == 'bench'",
        },
        {
          name: "Create source tarballs (release, linux)",
          if: [
            "matrix.os == 'linux' &&",
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
        {
          name: "Cache Cargo home",
          uses: "cirruslabs/cache@v4",
          with: {
            // See https://doc.rust-lang.org/cargo/guide/cargo-home.html#caching-the-cargo-home-in-ci
            // Note that with the new sparse registry format, we no longer have to cache a `.git` dir
            path: [
              "~/.cargo/.crates.toml",
              "~/.cargo/.crates2.json",
              "~/.cargo/bin",
              "~/.cargo/registry/index",
              "~/.cargo/registry/cache",
              "~/.cargo/git/db",
            ].join("\n"),
            key:
              `${cacheVersion}-cargo-home-\${{ matrix.os }}-\${{ matrix.arch }}-\${{ hashFiles('Cargo.lock') }}`,
            // We will try to restore from the closest cargo-home we can find
            "restore-keys":
              `${cacheVersion}-cargo-home-\${{ matrix.os }}-\${{ matrix.arch }}-`,
          },
        },
        installRustStep,
        {
          if:
            "matrix.job == 'lint' || matrix.job == 'test' || matrix.job == 'bench'",
          ...installDenoStep,
        },
        ...installPythonSteps.map((s) =>
          withCondition(
            s,
            "matrix.job != 'lint' && (matrix.os != 'linux' || matrix.arch != 'aarch64')",
          )
        ),
        {
          if: "matrix.job == 'bench' || matrix.job == 'test'",
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
            "matrix.os != 'windows' &&",
            "matrix.profile == 'release' &&",
            "matrix.job == 'test' &&",
            "github.repository == 'denoland/deno' &&",
            "(github.ref == 'refs/heads/main' ||",
            "startsWith(github.ref, 'refs/tags/'))",
          ].join("\n"),
          uses: "google-github-actions/setup-gcloud@v2",
          with: {
            project_id: "denoland",
          },
        },
        {
          name: "Setup gcloud (windows)",
          if: [
            "matrix.os == 'windows' &&",
            "matrix.profile == 'release' &&",
            "matrix.job == 'test' &&",
            "github.repository == 'denoland/deno' &&",
            "(github.ref == 'refs/heads/main' ||",
            "startsWith(github.ref, 'refs/tags/'))",
          ].join("\n"),
          uses: "google-github-actions/setup-gcloud@v2",
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
          run: 'echo "DENO_CANARY=true" >> $GITHUB_ENV',
        },
        {
          if: "matrix.use_sysroot",
          ...sysRootStep,
        },
        {
          name: "Remove macOS cURL --ipv4 flag",
          run: [
            // cURL's --ipv4 flag is busted for now
            "curl --version",
            "which curl",
            "cat /etc/hosts",
            "rm ~/.curlrc || true",
          ].join("\n"),
          if: `matrix.os == 'macos'`,
        },
        {
          name: "Install macOS aarch64 lld",
          run: [
            "./tools/install_prebuilt.js ld64.lld",
          ].join("\n"),
          if: `matrix.os == 'macos' && matrix.arch == 'aarch64'`,
        },
        {
          name: "Install rust-codesign",
          run: [
            "./tools/install_prebuilt.js rcodesign",
            "echo $GITHUB_WORKSPACE/third_party/prebuilt/mac >> $GITHUB_PATH",
          ].join("\n"),
          if: `matrix.os == 'macos'`,
        },
        {
          name: "Log versions",
          run: [
            "echo '*** Python'",
            "command -v python && python --version || echo 'No python found or bad executable'",
            "echo '*** Rust'",
            "command -v rustc && rustc --version || echo 'No rustc found or bad executable'",
            "echo '*** Cargo'",
            "command -v cargo && cargo --version || echo 'No cargo found or bad executable'",
            "echo '*** Deno'",
            "command -v deno && deno --version || echo 'No deno found or bad executable'",
            "echo '*** Node'",
            "command -v node && node --version || echo 'No node found or bad executable'",
            "echo '*** Installed packages'",
            "command -v dpkg && dpkg -l || echo 'No dpkg found or bad executable'",
          ].join("\n"),
        },
        {
          name: "Install benchmark tools",
          if: "matrix.job == 'bench'",
          run: [
            installBenchTools,
          ].join("\n"),
        },
        {
          // Restore cache from the latest 'main' branch build.
          name: "Restore cache build output (PR)",
          uses: "actions/cache/restore@v4",
          if:
            "github.ref != 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')",
          with: {
            path: prCachePath,
            key: "never_saved",
            "restore-keys": prCacheKeyPrefix,
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
          name: "Set up playwright cache",
          uses: "actions/cache@v4",
          with: {
            path: "./.ms-playwright",
            key: "playwright-${{ runner.os }}-${{ runner.arch }}",
          },
        },
        {
          name: "test_format.js",
          if: "matrix.job == 'lint' && matrix.os == 'linux'",
          run:
            "deno run --allow-write --allow-read --allow-run --allow-net ./tools/format.js --check",
        },
        {
          name: "Lint PR title",
          if:
            "matrix.job == 'lint' && github.event_name == 'pull_request' && matrix.os == 'linux'",
          env: {
            PR_TITLE: "${{ github.event.pull_request.title }}",
          },
          run: 'deno run ./tools/verify_pr_title.js "$PR_TITLE"',
        },
        {
          name: "lint.js",
          if: "matrix.job == 'lint'",
          run:
            "deno run --allow-write --allow-read --allow-run --allow-net ./tools/lint.js",
        },
        {
          name: "jsdoc_checker.js",
          if: "matrix.job == 'lint'",
          run:
            "deno run --allow-read --allow-env --allow-sys ./tools/jsdoc_checker.js",
        },
        {
          name: "node_compat/setup.ts --check",
          if: "matrix.job == 'lint' && matrix.os == 'linux'",
          run:
            "deno run --allow-write --allow-read --allow-run=git ./tests/node_compat/runner/setup.ts --check",
        },
        {
          name: "Check tracing build",
          if:
            "matrix.job == 'test' && matrix.profile == 'debug' && matrix.os == 'linux' && matrix.arch == 'x86_64'",
          run: "cargo check -p deno --features=lsp-tracing",
          env: { CARGO_PROFILE_DEV_DEBUG: 0 },
        },
        {
          name: "Build debug",
          if: "matrix.job == 'test' && matrix.profile == 'debug'",
          run: "cargo build --locked --all-targets --features=panic-trace",
          env: { CARGO_PROFILE_DEV_DEBUG: 0 },
        },
        // Uncomment for remote debugging
        // {
        //   name: "Setup tmate session",
        //   if: [
        //     "(matrix.job == 'test' || matrix.job == 'bench') &&",
        //     "matrix.profile == 'release' && (matrix.use_sysroot ||",
        //     "github.repository == 'denoland/deno')",
        //   ].join("\n"),
        //   uses: "mxschmitt/action-tmate@v3",
        // },
        {
          name: "Build release",
          if: [
            "(matrix.job == 'test' || matrix.job == 'bench') &&",
            "matrix.profile == 'release' && (matrix.use_sysroot ||",
            "github.repository == 'denoland/deno')",
          ].join("\n"),
          run: [
            // output fs space before and after building
            "df -h",
            "cargo build --release --locked --all-targets --features=panic-trace",
            "df -h",
          ].join("\n"),
        },
        {
          // Run a minimal check to ensure that binary is not corrupted, regardless
          // of our build mode
          name: "Check deno binary",
          if: "matrix.job == 'test'",
          run:
            'target/${{ matrix.profile }}/deno eval "console.log(1+2)" | grep 3',
          env: {
            NO_COLOR: 1,
          },
        },
        {
          // Verify that the binary actually works in the Ubuntu-16.04 sysroot.
          name: "Check deno binary (in sysroot)",
          if: "matrix.job == 'test' && matrix.use_sysroot",
          run:
            'sudo chroot /sysroot "$(pwd)/target/${{ matrix.profile }}/deno" --version',
        },
        {
          name: "Generate symcache",
          if: [
            "(matrix.job == 'test' || matrix.job == 'bench') &&",
            "matrix.profile == 'release' && (matrix.use_sysroot ||",
            "github.repository == 'denoland/deno')",
          ].join("\n"),
          run: [
            "target/release/deno -A tools/release/create_symcache.ts ./deno.symcache",
            "du -h deno.symcache",
            "du -h target/release/deno",
          ].join("\n"),
          env: {
            NO_COLOR: 1,
          },
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
          uses: "actions/upload-artifact@v4",
          with: {
            name:
              "deno-${{ matrix.os }}-${{ matrix.arch }}-${{ github.event.number }}",
            path: "target/release/deno",
          },
        },
        {
          name: "Pre-release (linux)",
          if: [
            "matrix.os == 'linux' &&",
            "(matrix.job == 'test' || matrix.job == 'bench') &&",
            "matrix.profile == 'release' &&",
            "github.repository == 'denoland/deno'",
          ].join("\n"),
          run: [
            "cd target/release",
            "./deno -A ../../tools/release/create_symcache.ts deno-${{ matrix.arch }}-unknown-linux-gnu.symcache",
            "strip ./deno",
            "zip -r deno-${{ matrix.arch }}-unknown-linux-gnu.zip deno",
            "shasum -a 256 deno-${{ matrix.arch }}-unknown-linux-gnu.zip > deno-${{ matrix.arch }}-unknown-linux-gnu.zip.sha256sum",
            "strip ./denort",
            "zip -r denort-${{ matrix.arch }}-unknown-linux-gnu.zip denort",
            "shasum -a 256 denort-${{ matrix.arch }}-unknown-linux-gnu.zip > denort-${{ matrix.arch }}-unknown-linux-gnu.zip.sha256sum",
            "./deno types > lib.deno.d.ts",
          ].join("\n"),
        },
        {
          name: "Pre-release (mac)",
          if: [
            `matrix.os == 'macos' &&`,
            "matrix.job == 'test' &&",
            "matrix.profile == 'release' &&",
            "github.repository == 'denoland/deno'",
          ].join("\n"),
          env: {
            "APPLE_CODESIGN_KEY": "${{ secrets.APPLE_CODESIGN_KEY }}",
            "APPLE_CODESIGN_PASSWORD": "${{ secrets.APPLE_CODESIGN_PASSWORD }}",
          },
          run: [
            "target/release/deno -A tools/release/create_symcache.ts target/release/deno-${{ matrix.arch }}-apple-darwin.symcache",
            "strip -x -S target/release/deno",
            'echo "Key is $(echo $APPLE_CODESIGN_KEY | base64 -d | wc -c) bytes"',
            "rcodesign sign target/release/deno " +
            "--code-signature-flags=runtime " +
            '--p12-password="$APPLE_CODESIGN_PASSWORD" ' +
            "--p12-file=<(echo $APPLE_CODESIGN_KEY | base64 -d) " +
            "--entitlements-xml-file=cli/entitlements.plist",
            "cd target/release",
            "zip -r deno-${{ matrix.arch }}-apple-darwin.zip deno",
            "shasum -a 256 deno-${{ matrix.arch }}-apple-darwin.zip > deno-${{ matrix.arch }}-apple-darwin.zip.sha256sum",
            "strip -x -S ./denort",
            "zip -r denort-${{ matrix.arch }}-apple-darwin.zip denort",
            "shasum -a 256 denort-${{ matrix.arch }}-apple-darwin.zip > denort-${{ matrix.arch }}-apple-darwin.zip.sha256sum",
          ]
            .join("\n"),
        },
        {
          name: "Pre-release (windows)",
          if: [
            "matrix.os == 'windows' &&",
            "matrix.job == 'test' &&",
            "matrix.profile == 'release' &&",
            "github.repository == 'denoland/deno'",
          ].join("\n"),
          shell: "pwsh",
          run: [
            "Compress-Archive -CompressionLevel Optimal -Force -Path target/release/deno.exe -DestinationPath target/release/deno-${{ matrix.arch }}-pc-windows-msvc.zip",
            "Get-FileHash target/release/deno-${{ matrix.arch }}-pc-windows-msvc.zip -Algorithm SHA256 | Format-List > target/release/deno-${{ matrix.arch }}-pc-windows-msvc.zip.sha256sum",
            "Compress-Archive -CompressionLevel Optimal -Force -Path target/release/denort.exe -DestinationPath target/release/denort-${{ matrix.arch }}-pc-windows-msvc.zip",
            "Get-FileHash target/release/denort-${{ matrix.arch }}-pc-windows-msvc.zip -Algorithm SHA256 | Format-List > target/release/denort-${{ matrix.arch }}-pc-windows-msvc.zip.sha256sum",
            "target/release/deno.exe -A tools/release/create_symcache.ts target/release/deno-${{ matrix.arch }}-pc-windows-msvc.symcache",
          ].join("\n"),
        },
        {
          name: "Upload canary to dl.deno.land",
          if: [
            "matrix.job == 'test' &&",
            "matrix.profile == 'release' &&",
            "github.repository == 'denoland/deno' &&",
            "github.ref == 'refs/heads/main'",
          ].join("\n"),
          run: [
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/canary/$(git rev-parse HEAD)/',
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.sha256sum gs://dl.deno.land/canary/$(git rev-parse HEAD)/',
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.symcache gs://dl.deno.land/canary/$(git rev-parse HEAD)/',
            "echo ${{ github.sha }} > canary-latest.txt",
            'gsutil -h "Cache-Control: no-cache" cp canary-latest.txt gs://dl.deno.land/canary-$(rustc -vV | sed -n "s|host: ||p")-latest.txt',
          ].join("\n"),
        },
        {
          name: "Autobahn testsuite",
          if: [
            "(matrix.os == 'linux' && matrix.arch != 'aarch64') &&",
            "matrix.job == 'test' &&",
            "matrix.profile == 'release' &&",
            "!startsWith(github.ref, 'refs/tags/')",
          ].join("\n"),
          run:
            "target/release/deno run -A --config tests/config/deno.json ext/websocket/autobahn/fuzzingclient.js",
        },
        {
          name: "Test (full, debug)",
          if: [
            "matrix.job == 'test' &&",
            "matrix.profile == 'debug' &&",
            "!startsWith(github.ref, 'refs/tags/') &&",
            // Run full tests only on Linux.
            "matrix.os == 'linux'",
          ].join("\n"),
          run: "cargo test --locked --features=panic-trace",
          env: { CARGO_PROFILE_DEV_DEBUG: 0 },
        },
        {
          name: "Test (fast, debug)",
          if: [
            "matrix.job == 'test' &&",
            "matrix.profile == 'debug' &&",
            "(startsWith(github.ref, 'refs/tags/') || matrix.os != 'linux')",
          ].join("\n"),
          run: [
            // Run unit then integration tests. Skip doc tests here
            // since they are sometimes very slow on Mac.
            "cargo test --locked --lib --features=panic-trace",
            "cargo test --locked --tests --features=panic-trace",
          ].join("\n"),
          env: { CARGO_PROFILE_DEV_DEBUG: 0 },
        },
        {
          name: "Test (release)",
          if: [
            "matrix.job == 'test' &&",
            "matrix.profile == 'release' &&",
            "(matrix.use_sysroot || (",
            "github.repository == 'denoland/deno' &&",
            "!startsWith(github.ref, 'refs/tags/')))",
          ].join("\n"),
          run: "cargo test --release --locked --features=panic-trace",
        },
        {
          name: "Configure hosts file for WPT",
          if: "matrix.wpt",
          run: "./wpt make-hosts-file | sudo tee -a /etc/hosts",
          "working-directory": "tests/wpt/suite/",
        },
        {
          name: "Run web platform tests (debug)",
          if: "matrix.wpt && matrix.profile == 'debug'",
          env: {
            DENO_BIN: "./target/debug/deno",
          },
          run: [
            "deno run -A --lock=tools/deno.lock.json --config tests/config/deno.json\\",
            "        ./tests/wpt/wpt.ts setup",
            "deno run -A --lock=tools/deno.lock.json --config tests/config/deno.json\\",
            '         ./tests/wpt/wpt.ts run --quiet --binary="$DENO_BIN"',
          ].join("\n"),
        },
        {
          name: "Run web platform tests (release)",
          if: "matrix.wpt && matrix.profile == 'release'",
          env: {
            DENO_BIN: "./target/release/deno",
          },
          run: [
            "deno run -A --lock=tools/deno.lock.json --config tests/config/deno.json\\",
            "         ./tests/wpt/wpt.ts setup",
            "deno run -A --lock=tools/deno.lock.json --config tests/config/deno.json\\",
            "         ./tests/wpt/wpt.ts run --quiet --release         \\",
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
            "matrix.os == 'linux' &&",
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
            "matrix.os == 'linux' &&",
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
          if: "matrix.job == 'bench' && !startsWith(github.ref, 'refs/tags/')",
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
            "./target/release/deno run --allow-all ./tools/build_benchmark_jsons.js --release",
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
            "matrix.job != 'lint' && matrix.profile != 'debug' && github.repository == 'denoland/deno' && (github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))",
          run: [
            'du -hd1 "./target/${{ matrix.profile }}"',
            'du -ha  "./target/${{ matrix.profile }}/deno"',
            'du -ha  "./target/${{ matrix.profile }}/denort"',
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
            "matrix.os != 'windows' &&",
            "matrix.job == 'test' &&",
            "matrix.profile == 'release' &&",
            "github.repository == 'denoland/deno' &&",
            "startsWith(github.ref, 'refs/tags/')",
          ].join("\n"),
          run: [
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.sha256sum gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.symcache gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
          ].join("\n"),
        },
        {
          name: "Upload release to dl.deno.land (windows)",
          if: [
            "matrix.os == 'windows' &&",
            "matrix.job == 'test' &&",
            "matrix.profile == 'release' &&",
            "github.repository == 'denoland/deno' &&",
            "startsWith(github.ref, 'refs/tags/')",
          ].join("\n"),
          env: {
            CLOUDSDK_PYTHON: "${{env.pythonLocation}}\\python.exe",
          },
          run: [
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.sha256sum gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.symcache gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
          ].join("\n"),
        },
        {
          name: "Create release notes",
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
              "target/release/deno-x86_64-pc-windows-msvc.zip.sha256sum",
              "target/release/denort-x86_64-pc-windows-msvc.zip",
              "target/release/denort-x86_64-pc-windows-msvc.zip.sha256sum",
              "target/release/deno-x86_64-unknown-linux-gnu.zip",
              "target/release/deno-x86_64-unknown-linux-gnu.zip.sha256sum",
              "target/release/denort-x86_64-unknown-linux-gnu.zip",
              "target/release/denort-x86_64-unknown-linux-gnu.zip.sha256sum",
              "target/release/deno-x86_64-apple-darwin.zip",
              "target/release/deno-x86_64-apple-darwin.zip.sha256sum",
              "target/release/denort-x86_64-apple-darwin.zip",
              "target/release/denort-x86_64-apple-darwin.zip.sha256sum",
              "target/release/deno-aarch64-unknown-linux-gnu.zip",
              "target/release/deno-aarch64-unknown-linux-gnu.zip.sha256sum",
              "target/release/denort-aarch64-unknown-linux-gnu.zip",
              "target/release/denort-aarch64-unknown-linux-gnu.zip.sha256sum",
              "target/release/deno-aarch64-apple-darwin.zip",
              "target/release/deno-aarch64-apple-darwin.zip.sha256sum",
              "target/release/denort-aarch64-apple-darwin.zip",
              "target/release/denort-aarch64-apple-darwin.zip.sha256sum",
              "target/release/deno_src.tar.gz",
              "target/release/lib.deno.d.ts",
            ].join("\n"),
            body_path: "target/release/release-notes.md",
            draft: true,
          },
        },
        {
          // In main branch, always create a fresh cache
          name: "Save cache build output (main)",
          uses: "actions/cache/save@v4",
          if:
            "(matrix.job == 'test' || matrix.job == 'lint') && github.ref == 'refs/heads/main'",
          with: {
            path: prCachePath,
            key: prCacheKey,
          },
        },
      ]),
    },
    wasm: {
      name: "build wasm32",
      needs: ["pre_build"],
      if: "${{ needs.pre_build.outputs.skip_build != 'true' }}",
      "runs-on": ubuntuX86Runner,
      "timeout-minutes": 30,
      steps: skipJobsIfPrAndMarkedSkip([
        ...cloneRepoStep,
        installRustStep,
        {
          name: "Install wasm target",
          run: "rustup target add wasm32-unknown-unknown",
        },
        {
          name: "Cargo build",
          // we want this crate to be wasm compatible
          run: "cargo build --target wasm32-unknown-unknown -p deno_resolver",
        },
      ]),
    },
    "publish-canary": {
      name: "publish canary",
      "runs-on": ubuntuX86Runner,
      needs: ["build"],
      if:
        "github.repository == 'denoland/deno' && github.ref == 'refs/heads/main'",
      steps: [
        authenticateWithGoogleCloud,
        {
          name: "Setup gcloud",
          uses: "google-github-actions/setup-gcloud@v2",
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

export function generate() {
  let finalText = `# GENERATED BY ./ci.generate.ts -- DO NOT DIRECTLY EDIT\n\n`;
  finalText += stringify(ci, {
    noRefs: true,
    lineWidth: 10_000,
    noCompatMode: true,
  });
  return finalText;
}

export const CI_YML_URL = new URL("./ci.yml", import.meta.url);

if (import.meta.main) {
  Deno.writeTextFileSync(CI_YML_URL, generate());
}
