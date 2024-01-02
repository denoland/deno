#!/usr/bin/env -S deno run --allow-write=. --lock=./tools/deno.lock.json
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import * as yaml from "https://deno.land/std@0.173.0/encoding/yaml.ts";

// Bump this number when you want to purge the cache.
// Note: the tools/release/01_bump_crate_versions.ts script will update this version
// automatically via regex, so ensure that this line maintains this format.
const cacheVersion = 66;

const ubuntuRunner = "ubuntu-22.04";
const ubuntuXlRunner = "ubuntu-22.04-xl";
const windowsRunner = "windows-2022";
const windowsXlRunner = "windows-2022-xl";
const macosX86Runner = "macos-12";
// https://github.blog/2023-10-02-introducing-the-new-apple-silicon-powered-m1-macos-larger-runner-for-github-actions/
const macosArmRunner = "macos-13-xlarge";

const Runners = (() => {
  return {
    ubuntuXl:
      `\${{ github.repository == 'denoland/deno' && '${ubuntuXlRunner}' || '${ubuntuRunner}' }}`,
    ubuntu: ubuntuRunner,
    linux: ubuntuRunner,
    macos: macosX86Runner,
    macosArm: macosArmRunner,
    windows: windowsRunner,
    windowsXl:
      `\${{ github.repository == 'denoland/deno' && '${windowsXlRunner}' || '${windowsRunner}' }}`,
  };
})();
const prCacheKeyPrefix =
  `${cacheVersion}-cargo-target-\${{ matrix.os }}-\${{ matrix.profile }}-\${{ matrix.job }}-`;

// Note that you may need to add more version to the `apt-get remove` line below if you change this
const llvmVersion = 16;
const installPkgsCommand =
  `sudo apt-get install --no-install-recommends debootstrap clang-${llvmVersion} lld-${llvmVersion} clang-tools-${llvmVersion} clang-format-${llvmVersion} clang-tidy-${llvmVersion}`;
const sysRootStep = {
  name: "Set up incremental LTO and sysroot build",
  run: `# Avoid running man-db triggers, which sometimes takes several minutes
# to complete.
sudo apt-get remove --purge -y man-db
# Remove older clang before we install
sudo apt-get remove 'clang-12*' 'clang-13*' 'clang-14*' 'clang-15*' 'llvm-12*' 'llvm-13*' 'llvm-14*' 'llvm-15*' 'lld-12*' 'lld-13*' 'lld-14*' 'lld-15*'

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
(yes '' | sudo update-alternatives --force --all) || true

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

wget https://github.com/denoland/deno_third_party/raw/master/prebuilt/linux64/libdl/libdl.a
wget https://github.com/denoland/deno_third_party/raw/master/prebuilt/linux64/libdl/libdl.so.2

sudo ln -s libdl.so.2 /sysroot/lib/x86_64-linux-gnu/libdl.so
sudo ln -s libdl.a /sysroot/lib/x86_64-linux-gnu/libdl.a

# Configure the build environment. Both Rust and Clang will produce
# llvm bitcode only, so we can use lld's incremental LTO support.
cat >> $GITHUB_ENV << __0
CARGO_PROFILE_BENCH_INCREMENTAL=false
CARGO_PROFILE_BENCH_LTO=false
CARGO_PROFILE_RELEASE_INCREMENTAL=false
CARGO_PROFILE_RELEASE_LTO=false
RUSTFLAGS<<__1
  -C linker-plugin-lto=true
  -C linker=clang-${llvmVersion}
  -C link-arg=-fuse-ld=lld-${llvmVersion}
  -C link-arg=--sysroot=/sysroot
  -C link-arg=-ldl
  -C link-arg=-Wl,--allow-shlib-undefined
  -C link-arg=-Wl,--thinlto-cache-dir=$(pwd)/target/release/lto-cache
  -C link-arg=-Wl,--thinlto-cache-policy,cache_size_bytes=700m
  --cfg tokio_unstable
  \${{ env.RUSTFLAGS }}
__1
RUSTDOCFLAGS<<__1
  -C linker-plugin-lto=true
  -C linker=clang-${llvmVersion}
  -C link-arg=-fuse-ld=lld-${llvmVersion}
  -C link-arg=--sysroot=/sysroot
  -C link-arg=-ldl
  -C link-arg=-Wl,--allow-shlib-undefined
  -C link-arg=-Wl,--thinlto-cache-dir=$(pwd)/target/release/lto-cache
  -C link-arg=-Wl,--thinlto-cache-policy,cache_size_bytes=700m
  \${{ env.RUSTFLAGS }}
__1
CC=clang-${llvmVersion}
CFLAGS=-flto=thin --sysroot=/sysroot
__0`,
};

const installBenchTools = "./tools/install_prebuilt.js wrk hyperfine";

// The Windows builder is a little strange -- there's lots of room on C: and not so much on D:
// We'll check out to D:, but then all of our builds should happen on a C:-mapped drive
const reconfigureWindowsStorage = {
  name: "Reconfigure Windows Storage",
  if: [
    "startsWith(matrix.os, 'windows') && !endsWith(matrix.os, '-xl')",
  ],
  shell: "pwsh",
  run: `
New-Item -ItemType "directory" -Path "$env:TEMP/__target__"
New-Item -ItemType Junction -Target "$env:TEMP/__target__" -Path "D:/a/deno/deno"
`.trim(),
};

const cloneRepoStep = [{
  name: "Configure git",
  run: [
    "git config --global core.symlinks true",
    "git config --global fetch.parallel 32",
  ].join("\n"),
}, {
  name: "Clone repository",
  uses: "actions/checkout@v3",
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
const installProtocStep = {
  name: "Install protoc",
  uses: "arduino/setup-protoc@v2",
  with: { "version": "21.12", "repo-token": "${{ secrets.GITHUB_TOKEN }}" },
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
  os: string;
  profile?: string;
  job?: string;
  use_sysroot?: boolean;
  wpt?: string;
}[]) {
  function getOsDisplayName(os: string) {
    if (os.includes("ubuntu")) {
      return "ubuntu-x86_64";
    } else if (os.includes("windows")) {
      return "windows-x86_64";
    } else if (os == macosX86Runner) {
      return "macos-x86_64";
    } else if (os == macosArmRunner) {
      return "macos-aarch64";
    } else {
      throw new Error(`Display name not found: ${os}`);
    }
  }

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
      let text =
        "${{ (!contains(github.event.pull_request.labels.*.name, 'ci-full') && (";
      text += removeSurroundingExpression(item.skip.toString()) + ")) && ";
      text += `'${Runners.ubuntu}' || ${
        removeSurroundingExpression(item.os)
      } }}`;

      // deno-lint-ignore no-explicit-any
      (item as any).runner = text;
      item.skip =
        "${{ !contains(github.event.pull_request.labels.*.name, 'ci-full') && (" +
        removeSurroundingExpression(item.skip.toString()) + ") }}";
    }

    return {
      ...item,
      os_display_name: getOsDisplayName(item.os),
    };
  });
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
          run: [
            "GIT_MESSAGE=$(git log --format=%s -n 1 ${{github.event.after}})",
            "echo Commit message: $GIT_MESSAGE",
            "echo $GIT_MESSAGE | grep '\\[ci\\]' || (echo 'Exiting due to draft PR. Commit with [ci] to bypass.' ; echo 'skip_build=true' >> $GITHUB_OUTPUT)",
          ].join("\n"),
        },
      ]),
    },
    build: {
      name:
        "${{ matrix.job }} ${{ matrix.profile }} ${{ matrix.os_display_name }}",
      needs: ["pre_build"],
      if: "${{ needs.pre_build.outputs.skip_build != 'true' }}",
      "runs-on": "${{ matrix.runner || matrix.os }}",
      "timeout-minutes": 120,
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
            os: Runners.macos,
            job: "test",
            profile: "debug",
          }, {
            os: Runners.macos,
            job: "test",
            profile: "release",
            skip_pr: true,
          }, {
            os: Runners.macosArm,
            job: "test",
            profile: "release",
            // TODO(mmastrac): We don't want to run this M1 runner on every main commit because of the expense.
            skip:
              "${{ github.event_name == 'pull_request' || github.ref == 'refs/heads/main' }}",
          }, {
            os: Runners.windows,
            job: "test",
            profile: "debug",
          }, {
            os: Runners.windowsXl,
            job: "test",
            profile: "release",
            skip_pr: true,
          }, {
            os: Runners.ubuntuXl,
            job: "test",
            profile: "release",
            use_sysroot: true,
            // TODO(ry): Because CI is so slow on for OSX and Windows, we
            // currently run the Web Platform tests only on Linux.
            wpt: "${{ !startsWith(github.ref, 'refs/tags/') }}",
          }, {
            os: Runners.ubuntuXl,
            job: "bench",
            profile: "release",
            use_sysroot: true,
            skip_pr:
              "${{ !contains(github.event.pull_request.labels.*.name, 'ci-bench') }}",
          }, {
            os: Runners.ubuntu,
            job: "test",
            profile: "debug",
            use_sysroot: true,
          }, {
            os: Runners.ubuntu,
            job: "lint",
            profile: "debug",
          }, {
            os: Runners.macos,
            job: "lint",
            profile: "debug",
          }, {
            os: Runners.windows,
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
      },
      steps: skipJobsIfPrAndMarkedSkip([
        reconfigureWindowsStorage,
        ...cloneRepoStep,
        submoduleStep("./test_util/std"),
        {
          ...submoduleStep("./test_util/wpt"),
          if: "matrix.wpt",
        },
        {
          ...submoduleStep("./tools/node_compat/node"),
          if: "matrix.job == 'lint' && startsWith(matrix.os, 'ubuntu')",
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
          if:
            "matrix.job == 'lint' || matrix.job == 'test' || matrix.job == 'bench'",
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
        installProtocStep,
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
          run: 'echo "DENO_CANARY=true" >> $GITHUB_ENV',
        },
        {
          if: "matrix.use_sysroot",
          ...sysRootStep,
        },
        {
          name: "Install aarch64 lld",
          run: [
            "./tools/install_prebuilt.js ld64.lld",
          ].join("\n"),
          if: `matrix.os == '${macosArmRunner}'`,
        },
        {
          name: "Install rust-codesign",
          run: [
            "./tools/install_prebuilt.js rcodesign",
            "echo $GITHUB_WORKSPACE/third_party/prebuilt/mac >> $GITHUB_PATH",
          ].join("\n"),
          if:
            `(matrix.os == '${macosArmRunner}' || matrix.os == '${macosX86Runner}')`,
        },
        {
          name: "Log versions",
          run: [
            "python --version",
            "rustc --version",
            "cargo --version",
            "which dpkg && dpkg -l",
            // Deno is installed when linting or testing.
            'if [[ "${{ matrix.job }}" == "lint" ]] || [[ "${{ matrix.job }}" == "test" ]]; then',
            "  deno --version",
            "fi",
            // Node is installed for benchmarks.
            'if [ "${{ matrix.job }}" == "bench" ]',
            "then",
            "  node -v",
            // Install benchmark tools.
            "  " + installBenchTools,
            "fi",
          ].join("\n"),
        },
        {
          name: "Cache Cargo home",
          uses: "actions/cache@v3",
          with: {
            // See https://doc.rust-lang.org/cargo/guide/cargo-home.html#caching-the-cargo-home-in-ci
            // Note that with the new sparse registry format, we no longer have to cache a `.git` dir
            path: [
              "~/.cargo/registry/index",
              "~/.cargo/registry/cache",
            ].join("\n"),
            key:
              `${cacheVersion}-cargo-home-\${{ matrix.os }}-\${{ hashFiles('Cargo.lock') }}`,
            // We will try to restore from the closest cargo-home we can find
            "restore-keys": `${cacheVersion}-cargo-home-\${{ matrix.os }}`,
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
          name: "test_format.js",
          if: "matrix.job == 'lint' && startsWith(matrix.os, 'ubuntu')",
          run:
            "deno run --unstable --allow-write --allow-read --allow-run --allow-net ./tools/format.js --check",
        },
        {
          name: "Lint PR title",
          if:
            "matrix.job == 'lint' && github.event_name == 'pull_request' && startsWith(matrix.os, 'ubuntu')",
          env: {
            PR_TITLE: "${{ github.event.pull_request.title }}",
          },
          run: 'deno run ./tools/verify_pr_title.js "$PR_TITLE"',
        },
        {
          name: "lint.js",
          if: "matrix.job == 'lint'",
          run:
            "deno run --unstable --allow-write --allow-read --allow-run --allow-net ./tools/lint.js",
        },
        {
          name: "node_compat/setup.ts --check",
          if: "matrix.job == 'lint' && startsWith(matrix.os, 'ubuntu')",
          run:
            "deno run --allow-write --allow-read --allow-run=git ./tools/node_compat/setup.ts --check",
        },
        {
          name: "Build debug",
          if: "matrix.job == 'test' && matrix.profile == 'debug'",
          run: [
            // output fs space before and after building
            "df -h",
            "cargo build --locked --all-targets",
            "df -h",
          ].join("\n"),
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
            "cargo build --release --locked --all-targets",
            "df -h",
          ].join("\n"),
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
          name: "Pre-release (mac intel)",
          if: [
            `matrix.os == '${macosX86Runner}' &&`,
            "matrix.job == 'test' &&",
            "matrix.profile == 'release' &&",
            "github.repository == 'denoland/deno'",
          ].join("\n"),
          env: {
            "APPLE_CODESIGN_KEY": "${{ secrets.APPLE_CODESIGN_KEY }}",
            "APPLE_CODESIGN_PASSWORD": "${{ secrets.APPLE_CODESIGN_PASSWORD }}",
          },
          run: [
            'echo "Key is $(echo $APPLE_CODESIGN_KEY | base64 -d | wc -c) bytes"',
            "rcodesign sign target/release/deno " +
            "--code-signature-flags=runtime " +
            '--p12-password="$APPLE_CODESIGN_PASSWORD" ' +
            "--p12-file=<(echo $APPLE_CODESIGN_KEY | base64 -d) " +
            "--entitlements-xml-file=cli/entitlements.plist",
            "cd target/release",
            "zip -r deno-x86_64-apple-darwin.zip deno",
          ]
            .join("\n"),
        },
        {
          name: "Pre-release (mac aarch64)",
          if: [
            `matrix.os == '${macosArmRunner}' &&`,
            "matrix.job == 'test' &&",
            "matrix.profile == 'release' &&",
            "github.repository == 'denoland/deno'",
          ].join("\n"),
          env: {
            "APPLE_CODESIGN_KEY": "${{ secrets.APPLE_CODESIGN_KEY }}",
            "APPLE_CODESIGN_PASSWORD": "${{ secrets.APPLE_CODESIGN_PASSWORD }}",
          },
          run: [
            'echo "Key is $(echo $APPLE_CODESIGN_KEY | base64 -d | wc -c) bytes"',
            "rcodesign sign target/release/deno " +
            "--code-signature-flags=runtime " +
            '--p12-password="$APPLE_CODESIGN_PASSWORD" ' +
            "--p12-file=<(echo $APPLE_CODESIGN_KEY | base64 -d) " +
            "--entitlements-xml-file=cli/entitlements.plist",
            "cd target/release",
            "zip -r deno-aarch64-apple-darwin.zip deno",
          ]
            .join("\n"),
        },
        {
          name: "Pre-release (windows)",
          if: [
            "startsWith(matrix.os, 'windows') &&",
            "matrix.job == 'test' &&",
            "matrix.profile == 'release' &&",
            "github.repository == 'denoland/deno'",
          ].join("\n"),
          shell: "pwsh",
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
          run:
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/canary/$(git rev-parse HEAD)/',
        },
        {
          name: "Autobahn testsuite",
          if: [
            "matrix.job == 'test' && matrix.profile == 'release' &&",
            "!startsWith(github.ref, 'refs/tags/') && startsWith(matrix.os, 'ubuntu')",
          ].join("\n"),
          run:
            "target/release/deno run -A --unstable ext/websocket/autobahn/fuzzingclient.js",
        },
        {
          name: "Test debug",
          if: [
            "matrix.job == 'test' && matrix.profile == 'debug' &&",
            "!startsWith(github.ref, 'refs/tags/') && startsWith(matrix.os, 'ubuntu')",
          ].join("\n"),
          run: "cargo test --locked",
          env: { CARGO_PROFILE_DEV_DEBUG: 0 },
        },
        {
          name: "Test debug (fast)",
          if: [
            "matrix.job == 'test' && matrix.profile == 'debug' &&",
            "(startsWith(github.ref, 'refs/tags/') || !startsWith(matrix.os, 'ubuntu'))",
          ].join("\n"),
          run: [
            // Run unit then integration tests. Skip doc tests here
            // since they are sometimes very slow on Mac.
            "cargo test --locked --lib",
            "cargo test --locked --test '*'",
          ].join("\n"),
          env: { CARGO_PROFILE_DEV_DEBUG: 0 },
        },
        {
          name: "Test release",
          if: [
            "matrix.job == 'test' && matrix.profile == 'release' &&",
            "(matrix.use_sysroot || (",
            "github.repository == 'denoland/deno' &&",
            "!startsWith(github.ref, 'refs/tags/')))",
          ].join("\n"),
          run: "cargo test --release --locked",
        },
        {
          // Since all tests are skipped when we're building a tagged commit
          // this is a minimal check to ensure that binary is not corrupted
          name: "Check deno binary",
          if:
            "matrix.profile == 'release' && startsWith(github.ref, 'refs/tags/')",
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
            "matrix.job != 'lint' && matrix.profile != 'debug' && github.repository == 'denoland/deno' && (github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))",
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
          run:
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
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
              "target/release/deno-x86_64-unknown-linux-gnu.zip",
              "target/release/deno-x86_64-apple-darwin.zip",
              "target/release/deno-aarch64-apple-darwin.zip",
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
          uses: "actions/cache/save@v3",
          if:
            "(matrix.job == 'test' || matrix.job == 'lint') && github.ref == 'refs/heads/main'",
          with: {
            path: [
              "./target",
              "!./target/*/gn_out",
              "!./target/*/*.zip",
              "!./target/*/*.tar.gz",
            ].join("\n"),
            key: prCacheKeyPrefix + "${{ github.sha }}",
          },
        },
      ]),
    },
    "publish-canary": {
      name: "publish canary",
      "runs-on": "ubuntu-22.04",
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
