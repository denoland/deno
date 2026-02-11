#!/usr/bin/env -S deno run --allow-write=. --allow-read=. --lock=./tools/deno.lock.json
// Copyright 2018-2026 the Deno authors. MIT license.
import { parse as parseToml } from "jsr:@std/toml@1";
import {
  createWorkflow,
  defineMatrix,
  job,
  step,
  steps,
} from "jsr:@david/gagen@0.1.2";

// Bump this number when you want to purge the cache.
// Note: the tools/release/01_bump_crate_versions.ts script will update this version
// automatically via regex, so ensure that this line maintains this format.
const cacheVersion = 94;

const ubuntuX86Runner = "ubuntu-24.04";
const ubuntuX86XlRunner = "ghcr.io/cirruslabs/ubuntu-runner-amd64:24.04";
const ubuntuARMRunner = "ghcr.io/cirruslabs/ubuntu-runner-arm64:24.04-plus";
const windowsX86Runner = "windows-2022";
const windowsX86XlRunner = "windows-2022-xl";
const windowsArmRunner = "windows-11-arm";
const macosX86Runner = "macos-15-intel";
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
  windowsArm: {
    os: "windows",
    arch: "aarch64",
    runner: windowsArmRunner,
  },
} as const;

// discover all non-binary, non-test workspace members for the libs test job
const libCrates = resolveLibCrates();
const libTestCrateArgs = libCrates.map((p) => `-p ${p}`).join(" ");
const libExcludeArgs = libCrates.map((p) => `--exclude ${p}`).join(" ");

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
const llvmVersion = 21;
const installPkgsCommand =
  `sudo apt-get install -y --no-install-recommends clang-${llvmVersion} lld-${llvmVersion} clang-tools-${llvmVersion} clang-format-${llvmVersion} clang-tidy-${llvmVersion}`;
const sysRootStepConfig = {
  name: "Set up incremental LTO and sysroot build",
  run: `# Setting up sysroot
export DEBIAN_FRONTEND=noninteractive
# Avoid running man-db triggers, which sometimes takes several minutes
# to complete.
sudo apt-get -qq remove --purge -y man-db > /dev/null 2> /dev/null
# Remove older clang before we install
sudo apt-get -qq remove \
  'clang-12*' 'clang-13*' 'clang-14*' 'clang-15*' 'clang-16*' 'clang-17*' 'clang-18*' 'clang-19*' 'llvm-12*' 'llvm-13*' 'llvm-14*' 'llvm-15*' 'llvm-16*' 'llvm-17*' 'llvm-18*' 'llvm-19*' 'lld-12*' 'lld-13*' 'lld-14*' 'lld-15*' 'lld-16*' 'lld-17*' 'lld-18*' 'lld-19*' > /dev/null 2> /dev/null

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
wget -q https://github.com/denoland/deno_sysroot_build/releases/download/sysroot-20250207/sysroot-\`uname -m\`.tar.xz -O /tmp/sysroot.tar.xz
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

      item.runner = runner;
      item.skip =
        "${{ !contains(github.event.pull_request.labels.*.name, 'ci-full') && (" +
        removeSurroundingExpression(item.skip.toString()) + ") }}";
    }

    return { ...item };
  });
}

// === pre_build job ===
// The pre_build step is used to skip running the CI on draft PRs and to not even
// start the build job. This can be overridden by adding [ci] to the commit title

const preBuildCheckStep = step({
  id: "check",
  if: "!contains(github.event.pull_request.labels.*.name, 'ci-draft')",
  run: [
    "GIT_MESSAGE=$(git log --format=%s -n 1 ${{github.event.after}})",
    "echo Commit message: $GIT_MESSAGE",
    "echo $GIT_MESSAGE | grep '\\[ci\\]' || (echo 'Exiting due to draft PR. Commit with [ci] to bypass or add the ci-draft label.' ; echo 'skip_build=true' >> $GITHUB_OUTPUT)",
  ],
  outputs: ["skip_build"] as const,
});

const preBuildJob = job("pre_build", {
  name: "pre-build",
  runsOn: "ubuntu-latest",
  steps: steps(
    {
      name: "Configure git",
      run: [
        "git config --global core.symlinks true",
        "git config --global fetch.parallel 32",
      ],
    },
    {
      name: "Clone repository",
      uses: "actions/checkout@v6",
      with: {
        // Use depth > 1, because sometimes we need to rebuild main and if
        // other commits have landed it will become impossible to rebuild if
        // the checkout is too shallow.
        "fetch-depth": 5,
        submodules: false,
      },
    },
    preBuildCheckStep,
  ).if("github.event.pull_request.draft == true"),
  outputs: { skip_build: preBuildCheckStep.outputs.skip_build },
});

// === build job ===

const buildMatrix = defineMatrix({
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
    ...Runners.windowsArm,
    job: "test",
    profile: "debug",
  }, {
    ...Runners.windowsArm,
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
    ...Runners.linuxArm,
    job: "test",
    profile: "debug",
  }, {
    ...Runners.linuxArm,
    job: "test",
    profile: "release",
    use_sysroot: true,
    skip_pr: true,
  }]),
});

const buildJob = job("build", {
  name:
    `${buildMatrix.job} ${buildMatrix.profile} ${buildMatrix.os}-${buildMatrix.arch}`,
  needs: [preBuildJob],
  if: preBuildJob.outputs.skip_build.notEquals("true"),
  runsOn: buildMatrix.runner,
  // This is required to successfully authenticate with Azure using OIDC for
  // code signing.
  environment: {
    name:
      "${{ (github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/')) && 'build' || '' }}",
  },
  timeoutMinutes: 240,
  defaults: {
    run: {
      // GH actions does not fail fast by default on
      // Windows, so we set bash as the default shell
      shell: "bash",
    },
  },
  strategy: {
    matrix: buildMatrix,
    // Always run main branch builds to completion. This allows the cache to
    // stay mostly up-to-date in situations where a single job fails due to
    // e.g. a flaky test.
    // Don't fast-fail on tag build because publishing binaries shouldn't be
    // prevented if any of the stages fail (which can be a false negative).
    failFast:
      "${{ github.event_name == 'pull_request' || (github.ref != 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')) }}",
  },
  env: {
    CARGO_TERM_COLOR: "always",
    RUST_BACKTRACE: "full",
    // disable anyhow's library backtrace
    RUST_LIB_BACKTRACE: 0,
  },
  steps: steps(
    // GitHub does not make skipping a specific matrix element easy
    // so just apply the !(matrix.skip) condition to all the steps.
    // https://stackoverflow.com/questions/65384420/how-to-make-a-github-action-matrix-element-conditional
    {
      name: "Configure git",
      run: [
        "git config --global core.symlinks true",
        "git config --global fetch.parallel 32",
      ],
    },
    {
      name: "Clone repository",
      uses: "actions/checkout@v6",
      with: { "fetch-depth": 5, submodules: false },
    },
    {
      name: "Clone submodule ./tests/util/std",
      run:
        "git submodule update --init --recursive --depth=1 -- ./tests/util/std",
    },
    {
      name: "Clone submodule ./tests/wpt/suite",
      if: "matrix.wpt",
      run:
        "git submodule update --init --recursive --depth=1 -- ./tests/wpt/suite",
    },
    {
      name: "Clone submodule ./tests/node_compat/runner/suite",
      if: "matrix.job == 'test'",
      run:
        "git submodule update --init --recursive --depth=1 -- ./tests/node_compat/runner/suite",
    },
    {
      name: "Clone submodule ./cli/bench/testdata/lsp_benchdata",
      if: "matrix.job == 'bench'",
      run:
        "git submodule update --init --recursive --depth=1 -- ./cli/bench/testdata/lsp_benchdata",
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
      ],
    },
    {
      name: "Cache Cargo home",
      uses: "cirruslabs/cache@v4",
      with: {
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
        "restore-keys":
          `${cacheVersion}-cargo-home-\${{ matrix.os }}-\${{ matrix.arch }}-`,
      },
    },
    { uses: "dsherret/rust-toolchain-file@v1" },
    {
      if: "matrix.os == 'linux' && matrix.arch == 'aarch64'",
      name: "Load 'vsock_loopback; kernel module",
      run: "sudo modprobe vsock_loopback",
    },
    {
      name: "Install Deno",
      uses: "denoland/setup-deno@v2",
      with: { "deno-version": "v2.x" },
      if:
        "(matrix.job == 'test' || matrix.job == 'bench') && !(matrix.os == 'windows' && matrix.arch == 'aarch64')",
    },
    {
      name: "Install Python",
      uses: "actions/setup-python@v6",
      with: { "python-version": 3.11 },
      if: "matrix.os != 'linux' || matrix.arch != 'aarch64'",
    },
    {
      name: "Remove unused versions of Python",
      if:
        "(matrix.os != 'linux' || matrix.arch != 'aarch64') && (matrix.os == 'windows')",
      shell: "pwsh",
      run: [
        '$env:PATH -split ";" |',
        '  Where-Object { Test-Path "$_\\python.exe" } |',
        "  Select-Object -Skip 1 |",
        '  ForEach-Object { Move-Item "$_" "$_.disabled" }',
      ],
    },
    {
      name: "Install Node",
      uses: "actions/setup-node@v6",
      with: { "node-version": 22 },
      if: "matrix.job == 'bench' || matrix.job == 'test'",
    },
    {
      name: "Authenticate with Google Cloud",
      uses: "google-github-actions/auth@v3",
      with: {
        "project_id": "denoland",
        "credentials_json": "${{ secrets.GCP_SA_KEY }}",
        "export_environment_variables": true,
        "create_credentials_file": true,
      },
      if: [
        "matrix.profile == 'release' &&",
        "matrix.job == 'test' &&",
        "github.repository == 'denoland/deno' &&",
        "(github.ref == 'refs/heads/main' ||",
        "startsWith(github.ref, 'refs/tags/'))",
      ].join("\n"),
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
      uses: "google-github-actions/setup-gcloud@v3",
      with: { project_id: "denoland" },
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
      env: { CLOUDSDK_PYTHON: "${{env.pythonLocation}}\\python.exe" },
      with: { project_id: "denoland" },
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
      ...sysRootStepConfig,
    },
    {
      name: "Remove macOS cURL --ipv4 flag",
      run: [
        // cURL's --ipv4 flag is busted for now
        "curl --version",
        "which curl",
        "cat /etc/hosts",
        "rm ~/.curlrc || true",
      ],
      if: "matrix.os == 'macos'",
    },
    {
      name: "Install macOS aarch64 lld",
      if: "matrix.os == 'macos' && matrix.arch == 'aarch64'",
      env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" },
      run: "./tools/install_prebuilt.js ld64.lld",
    },
    {
      name: "Install rust-codesign",
      env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" },
      run: "./tools/install_prebuilt.js rcodesign",
      if: "matrix.os == 'macos'",
    },
    {
      if: "matrix.os == 'macos'",
      env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" },
      run: "echo $GITHUB_WORKSPACE/third_party/prebuilt/mac >> $GITHUB_PATH",
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
      ],
    },
    {
      name: "Install benchmark tools",
      if: "matrix.job == 'bench'",
      env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" },
      run: installBenchTools,
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
      with: { "cache-path": "./target" },
    },
    {
      name: "Set up playwright cache",
      uses: "actions/cache@v5",
      with: {
        path: "./.ms-playwright",
        key: "playwright-${{ runner.os }}-${{ runner.arch }}",
      },
    },
    {
      name: "Build debug",
      if: "matrix.job == 'test' && matrix.profile == 'debug'",
      run: "cargo build --locked --all-targets --features=panic-trace",
      env: { CARGO_PROFILE_DEV_DEBUG: 0 },
    },
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
      ],
    },
    {
      // Run a minimal check to ensure that binary is not corrupted, regardless
      // of our build mode
      name: "Check deno binary",
      if: "matrix.job == 'test'",
      run: 'target/${{ matrix.profile }}/deno eval "console.log(1+2)" | grep 3',
      env: { NO_COLOR: 1 },
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
      ],
      env: { NO_COLOR: 1 },
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
      uses: "actions/upload-artifact@v6",
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
      ],
    },
    {
      name: "Pre-release (mac)",
      if: [
        "matrix.os == 'macos' &&",
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
      ],
    },
    {
      // Note: Azure OIDC credentials are only valid for 5 minutes, so
      // authentication must be done right before signing.
      name: "Authenticate with Azure (windows)",
      if: [
        "matrix.os == 'windows' &&",
        "matrix.job == 'test' &&",
        "matrix.profile == 'release' &&",
        "github.repository == 'denoland/deno' &&",
        "(github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))",
      ].join("\n"),
      uses: "azure/login@v1",
      with: {
        "client-id": "${{ secrets.AZURE_CLIENT_ID }}",
        "tenant-id": "${{ secrets.AZURE_TENANT_ID }}",
        "subscription-id": "${{ secrets.AZURE_SUBSCRIPTION_ID }}",
        "enable-AzPSSession": true,
      },
    },
    {
      name: "Code sign deno.exe (windows)",
      if: [
        "matrix.os == 'windows' &&",
        "matrix.job == 'test' &&",
        "matrix.profile == 'release' &&",
        "github.repository == 'denoland/deno' &&",
        "(github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))",
      ].join("\n"),
      uses: "Azure/artifact-signing-action@v0",
      with: {
        "endpoint": "https://eus.codesigning.azure.net/",
        "trusted-signing-account-name": "deno-cli-code-signing",
        "certificate-profile-name": "deno-cli-code-signing-cert",
        "files-folder": "target/release",
        "files-folder-filter": "deno.exe",
        "file-digest": "SHA256",
        "timestamp-rfc3161": "http://timestamp.acs.microsoft.com",
        "timestamp-digest": "SHA256",
        "exclude-environment-credential": true,
        "exclude-workload-identity-credential": true,
        "exclude-managed-identity-credential": true,
        "exclude-shared-token-cache-credential": true,
        "exclude-visual-studio-credential": true,
        "exclude-visual-studio-code-credential": true,
        "exclude-azure-cli-credential": false,
      },
    },
    {
      name: "Verify signature (windows)",
      if: [
        "matrix.os == 'windows' &&",
        "matrix.job == 'test' &&",
        "matrix.profile == 'release' &&",
        "github.repository == 'denoland/deno' &&",
        "(github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))",
      ].join("\n"),
      shell: "pwsh",
      run: [
        '$SignTool = Get-ChildItem -Path "C:\\Program Files*\\Windows Kits\\*\\bin\\*\\x64\\signtool.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1',
        "$SignToolPath = $SignTool.FullName",
        "& $SignToolPath verify /pa /v target\\release\\deno.exe",
      ],
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
      ],
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
        "rm canary-latest.txt gha-creds-*.json",
      ],
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
      run:
        `cargo test --workspace --locked ${libExcludeArgs} --features=panic-trace`,
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
        `cargo test --workspace --locked ${libExcludeArgs} --lib --features=panic-trace`,
        `cargo test --workspace --locked ${libExcludeArgs} --tests --features=panic-trace`,
      ],
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
      run:
        `cargo test --workspace --release --locked ${libExcludeArgs} --features=panic-trace`,
    },
    {
      name: "Ensure no git changes",
      if: "matrix.job == 'test' && github.event_name == 'pull_request'",
      run: [
        'if [[ -n "$(git status --porcelain)" ]]; then',
        'echo "❌ Git working directory is dirty. Ensure `cargo test` is not modifying git tracked files."',
        'echo ""',
        'echo "📋 Status:"',
        "git status",
        'echo ""',
        "exit 1",
        "fi",
      ],
    },
    {
      name: "Combine test results",
      if: [
        "always() &&",
        "matrix.job == 'test' &&",
        "!startsWith(github.ref, 'refs/tags/') &&",
        "!(matrix.os == 'windows' && matrix.arch == 'aarch64')",
      ].join("\n"),
      run: "deno run -RWN ./tools/combine_test_results.js",
    },
    {
      name: "Upload test results",
      uses: "actions/upload-artifact@v4",
      if: [
        "always() &&",
        "matrix.job == 'test' &&",
        "!startsWith(github.ref, 'refs/tags/') &&",
        "!(matrix.os == 'windows' && matrix.arch == 'aarch64')",
      ].join("\n"),
      with: {
        name:
          "test-results-${{ matrix.os }}-${{ matrix.arch }}-${{ matrix.profile }}.json",
        path: "target/test_results.json",
      },
    },
    {
      name: "Configure hosts file for WPT",
      if: "matrix.wpt",
      run: "./wpt make-hosts-file | sudo tee -a /etc/hosts",
      workingDirectory: "tests/wpt/suite/",
    },
    {
      name: "Run web platform tests (debug)",
      if: "matrix.wpt && matrix.profile == 'debug'",
      env: { DENO_BIN: "./target/debug/deno" },
      run: [
        "deno run -RWNE --allow-run --lock=tools/deno.lock.json --config tests/config/deno.json \\",
        "    ./tests/wpt/wpt.ts setup",
        "deno run -RWNE --allow-run --lock=tools/deno.lock.json --config tests/config/deno.json --unsafely-ignore-certificate-errors \\",
        '    ./tests/wpt/wpt.ts run --quiet --binary="$DENO_BIN"',
      ],
    },
    {
      name: "Run web platform tests (release)",
      if: "matrix.wpt && matrix.profile == 'release'",
      env: { DENO_BIN: "./target/release/deno" },
      run: [
        "deno run -RWNE --allow-run --lock=tools/deno.lock.json --config tests/config/deno.json \\",
        "    ./tests/wpt/wpt.ts setup",
        "deno run -RWNE --allow-run --lock=tools/deno.lock.json --config tests/config/deno.json --unsafely-ignore-certificate-errors \\",
        '    ./tests/wpt/wpt.ts run --quiet --release --binary="$DENO_BIN" --json=wpt.json --wptreport=wptreport.json',
      ],
    },
    {
      name: "Upload wpt results to dl.deno.land",
      continueOnError: true,
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
      ],
    },
    {
      name: "Upload wpt results to wpt.fyi",
      continueOnError: true,
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
      ],
    },
    {
      name: "Run benchmarks",
      if: "matrix.job == 'bench' && !startsWith(github.ref, 'refs/tags/')",
      run: "cargo bench --locked",
    },
    {
      name: "Post benchmarks",
      if: [
        "matrix.job == 'bench' &&",
        "github.repository == 'denoland/deno' &&",
        "github.ref == 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')",
      ].join("\n"),
      env: { DENOBOT_PAT: "${{ secrets.DENOBOT_PAT }}" },
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
      ],
    },
    {
      name: "Build product size info",
      if:
        "matrix.profile != 'debug' && github.repository == 'denoland/deno' && (github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))",
      run: [
        'du -hd1 "./target/${{ matrix.profile }}"',
        'du -ha  "./target/${{ matrix.profile }}/deno"',
        'du -ha  "./target/${{ matrix.profile }}/denort"',
      ],
    },
    {
      name: "Worker info",
      if: "matrix.job == 'bench'",
      run: ["cat /proc/cpuinfo", "cat /proc/meminfo"],
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
      ],
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
      env: { CLOUDSDK_PYTHON: "${{env.pythonLocation}}\\python.exe" },
      run: [
        'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
        'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.sha256sum gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
        'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.symcache gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
      ],
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
      ],
    },
    {
      name: "Upload release to GitHub",
      uses: "softprops/action-gh-release@v2",
      if: [
        "matrix.job == 'test' &&",
        "matrix.profile == 'release' &&",
        "github.repository == 'denoland/deno' &&",
        "startsWith(github.ref, 'refs/tags/')",
      ].join("\n"),
      env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" },
      with: {
        files: [
          "target/release/deno-x86_64-pc-windows-msvc.zip",
          "target/release/deno-x86_64-pc-windows-msvc.zip.sha256sum",
          "target/release/denort-x86_64-pc-windows-msvc.zip",
          "target/release/denort-x86_64-pc-windows-msvc.zip.sha256sum",
          "target/release/deno-aarch64-pc-windows-msvc.zip",
          "target/release/deno-aarch64-pc-windows-msvc.zip.sha256sum",
          "target/release/denort-aarch64-pc-windows-msvc.zip",
          "target/release/denort-aarch64-pc-windows-msvc.zip.sha256sum",
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
      if: "matrix.job == 'test' && github.ref == 'refs/heads/main'",
      with: { path: prCachePath, key: prCacheKey },
    },
  ).if("!(matrix.skip)"),
});

// === lint job ===

const lintMatrix = defineMatrix({
  include: [{
    ...Runners.linuxX86,
    profile: "debug",
    job: "lint",
  }, {
    ...Runners.macosX86,
    profile: "debug",
    job: "lint",
  }, {
    ...Runners.windowsX86,
    profile: "debug",
    job: "lint",
  }],
});

const lintJob = job("lint", {
  name: `lint ${lintMatrix.profile} ${lintMatrix.os}-${lintMatrix.arch}`,
  needs: [preBuildJob],
  if: preBuildJob.outputs.skip_build.notEquals("true"),
  runsOn: lintMatrix.runner,
  timeoutMinutes: 30,
  defaults: { run: { shell: "bash" } },
  strategy: {
    matrix: lintMatrix,
  },
  steps: steps(
    {
      name: "Configure git",
      run: [
        "git config --global core.symlinks true",
        "git config --global fetch.parallel 32",
      ],
    },
    {
      name: "Clone repository",
      uses: "actions/checkout@v6",
      with: { "fetch-depth": 5, submodules: false },
    },
    {
      name: "Clone submodule ./tests/util/std",
      run:
        "git submodule update --init --recursive --depth=1 -- ./tests/util/std",
    },
    {
      name: "Cache Cargo home",
      uses: "cirruslabs/cache@v4",
      with: {
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
        "restore-keys":
          `${cacheVersion}-cargo-home-\${{ matrix.os }}-\${{ matrix.arch }}-`,
      },
    },
    {
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
      uses: "dsherret/rust-toolchain-file@v1",
    },
    {
      name: "Install Deno",
      uses: "denoland/setup-deno@v2",
      with: { "deno-version": "v2.x" },
    },
    steps(
      {
        name: "test_format.js",
        run:
          "deno run --allow-write --allow-read --allow-run --allow-net ./tools/format.js --check",
      },
      {
        name: "jsdoc_checker.js",
        if: "matrix.os == 'linux'",
        run:
          "deno run --allow-read --allow-env --allow-sys ./tools/jsdoc_checker.js",
      },
    ).if("matrix.os == 'linux'"),
    {
      name: "lint.js",
      env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" },
      run:
        "deno run --allow-write --allow-read --allow-run --allow-net --allow-env ./tools/lint.js",
    },
    {
      name: "Save cache build output (main)",
      uses: "actions/cache/save@v4",
      if: "matrix.job == 'test' && github.ref == 'refs/heads/main'",
      with: { path: prCachePath, key: prCacheKey },
    },
  ),
});

// === libs job ===

const libsMatrix = defineMatrix({
  include: [{
    ...Runners.linuxX86,
    profile: "debug",
    job: "libs",
  }, {
    ...Runners.macosArm,
    profile: "debug",
    job: "libs",
  }, {
    ...Runners.windowsX86,
    profile: "debug",
    job: "libs",
  }],
});

const libsJob = job("libs", {
  name: `libs ${libsMatrix.profile} ${libsMatrix.os}-${libsMatrix.arch}`,
  needs: [preBuildJob],
  if: preBuildJob.outputs.skip_build.notEquals("true"),
  runsOn: libsMatrix.runner,
  timeoutMinutes: 30,
  strategy: {
    matrix: libsMatrix,
  },
  steps: steps(
    {
      name: "Configure git",
      run: [
        "git config --global core.symlinks true",
        "git config --global fetch.parallel 32",
      ],
    },
    {
      name: "Clone repository",
      uses: "actions/checkout@v6",
      with: { "fetch-depth": 5, submodules: false },
    },
    {
      name: "Clone submodule ./tests/util/std",
      run:
        "git submodule update --init --recursive --depth=1 -- ./tests/util/std",
    },
    {
      name: "Cache Cargo home",
      uses: "cirruslabs/cache@v4",
      with: {
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
        "restore-keys":
          `${cacheVersion}-cargo-home-\${{ matrix.os }}-\${{ matrix.arch }}-`,
      },
    },
    { uses: "dsherret/rust-toolchain-file@v1" },
    {
      if: "matrix.os == 'macos'",
      name: "Install Deno",
      uses: "denoland/setup-deno@v2",
      with: { "deno-version": "v2.x" },
    },
    {
      name: "Install macOS aarch64 lld",
      if: "matrix.os == 'macos' && matrix.arch == 'aarch64'",
      env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" },
      run: "./tools/install_prebuilt.js ld64.lld",
    },
    {
      if: "matrix.os == 'macos'",
      env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" },
      run: "echo $GITHUB_WORKSPACE/third_party/prebuilt/mac >> $GITHUB_PATH",
    },
    {
      name: "Install wasm target",
      if: "matrix.os == 'linux'",
      run: "rustup target add wasm32-unknown-unknown",
    },
    // we want these crates to be Wasm compatible
    {
      name: "Cargo check (deno_resolver)",
      if: "matrix.os == 'linux'",
      run:
        "cargo check --target wasm32-unknown-unknown -p deno_resolver && cargo check --target wasm32-unknown-unknown -p deno_resolver --features graph && cargo check --target wasm32-unknown-unknown -p deno_resolver --features graph --features deno_ast",
    },
    {
      name: "Cargo check (deno_npm_installer)",
      if: "matrix.os == 'linux'",
      run: "cargo check --target wasm32-unknown-unknown -p deno_npm_installer",
    },
    {
      name: "Cargo check (deno_config)",
      if: "matrix.os == 'linux'",
      run: [
        "cargo check --no-default-features -p deno_config",
        "cargo check --no-default-features --features workspace -p deno_config",
        "cargo check --no-default-features --features package_json -p deno_config",
        "cargo check --no-default-features --features workspace --features sync -p deno_config",
        "cargo check --target wasm32-unknown-unknown --all-features -p deno_config",
        "cargo check -p deno --features=lsp-tracing",
      ],
    },
    {
      name: "Test libs",
      run: `cargo test --locked ${libTestCrateArgs}`,
      env: { CARGO_PROFILE_DEV_DEBUG: 0 },
    },
  ).if("!(matrix.skip)"),
});

// === publish-canary job ===

const publishCanaryJob = job("publish-canary", {
  name: "publish canary",
  runsOn: ubuntuX86Runner,
  needs: [buildJob],
  if: "github.repository == 'denoland/deno' && github.ref == 'refs/heads/main'",
  steps: steps(
    {
      name: "Authenticate with Google Cloud",
      uses: "google-github-actions/auth@v3",
      with: {
        "project_id": "denoland",
        "credentials_json": "${{ secrets.GCP_SA_KEY }}",
        "export_environment_variables": true,
        "create_credentials_file": true,
      },
    },
    {
      name: "Setup gcloud",
      uses: "google-github-actions/setup-gcloud@v2",
      with: { project_id: "denoland" },
    },
    {
      name: "Upload canary version file to dl.deno.land",
      run: [
        "echo ${{ github.sha }} > canary-latest.txt",
        'gsutil -h "Cache-Control: no-cache" cp canary-latest.txt gs://dl.deno.land/canary-latest.txt',
      ],
    },
  ),
});

// === generate workflow ===

const workflow = createWorkflow({
  name: "ci",
  permissions: {
    contents: "write",
    "id-token": "write", // Required for GitHub OIDC with Azure for code signing
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
    cancelInProgress: true,
  },
  jobs: [preBuildJob, buildJob, lintJob, libsJob, publishCanaryJob],
});

export function generate() {
  return workflow.toYamlString({
    header: "# GENERATED BY ./ci.generate.ts -- DO NOT DIRECTLY EDIT",
  });
}

export const CI_YML_URL = new URL("./ci.yml", import.meta.url);

if (import.meta.main) {
  workflow.writeOrLint({
    filePath: CI_YML_URL,
    header: "# GENERATED BY ./ci.generate.ts -- DO NOT DIRECTLY EDIT",
  });
}

function resolveLibCrates() {
  // discover all non-binary, non-test workspace members for the libs test job
  const rootCargoToml = parseToml(
    Deno.readTextFileSync(new URL("../../Cargo.toml", import.meta.url)),
  ) as { workspace: { members: string[] } };

  const libCrates: string[] = [];
  for (const member of rootCargoToml.workspace.members) {
    // test crates depend on the deno binary at runtime
    if (member.startsWith("tests")) continue;

    const cargoToml = parseToml(
      Deno.readTextFileSync(
        new URL(`../../${member}/Cargo.toml`, import.meta.url),
      ),
    ) as { package: { name: string }; bin?: unknown[] };

    // skip binary crates (they need their own build step)
    if (cargoToml.bin) continue;

    libCrates.push(cargoToml.package.name);
  }
  return libCrates;
}
