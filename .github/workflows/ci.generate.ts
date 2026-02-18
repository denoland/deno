#!/usr/bin/env -S deno run --check --allow-write=. --allow-read=. --lock=./tools/deno.lock.json
// Copyright 2018-2026 the Deno authors. MIT license.
import { parse as parseToml } from "jsr:@std/toml@1";
import {
  Condition,
  conditions,
  createWorkflow,
  defineArtifact,
  defineExprObj,
  defineMatrix,
  type ExpressionValue,
  job,
  step,
} from "jsr:@david/gagen@0.2.16";

// Bump this number when you want to purge the cache.
// Note: the tools/release/01_bump_crate_versions.ts script will update this version
// automatically via regex, so ensure that this line maintains this format.
const cacheVersion = 97;

const ubuntuX86Runner = "ubuntu-24.04";
const ubuntuX86XlRunner = "ghcr.io/cirruslabs/ubuntu-runner-amd64:24.04";
const ubuntuARMXlRunner = "ghcr.io/cirruslabs/ubuntu-runner-arm64:24.04-plus";
const ubuntuARMRunner = "ubuntu-24.04-arm";
const windowsX86Runner = "windows-2022";
const windowsX86XlRunner = "windows-2022-xl";
const windowsArmRunner = "windows-11-arm";
const macosX86Runner = "macos-15-intel";
const macosArmRunner = "macos-14";
const selfHostedMacosArmRunner = "ghcr.io/cirruslabs/macos-runner:sonoma";

// shared conditions
const isDenoland = conditions.isRepository("denoland/deno");
const isMainBranch = conditions.isBranch("main");
const isTag = conditions.isTag();
const isNotTag = isTag.not();
const isMainOrTag = isMainBranch.or(isTag);
const isPr = conditions.isPr();
const hasCiFullLabel = conditions.hasPrLabel("ci-full");
const hasCiBenchLabel = conditions.hasPrLabel("ci-bench");

const Runners = {
  linuxX86: {
    os: "linux",
    arch: "x86_64",
    runner: ubuntuX86Runner,
  },
  linuxX86Xl: {
    os: "linux",
    arch: "x86_64",
    runner: isDenoland.then(ubuntuX86XlRunner).else(ubuntuX86Runner),
    testRunner: ubuntuX86Runner,
  },
  linuxArm: {
    os: "linux",
    arch: "aarch64",
    runner: ubuntuARMXlRunner,
    testRunner: ubuntuARMRunner,
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
    // actually use self-hosted runner only in denoland/deno on `main` branch and for tags (release) builds
    runner: isDenoland.and(isMainOrTag).then(selfHostedMacosArmRunner)
      .else(macosArmRunner),
    testRunner: macosArmRunner,
  },
  windowsX86: {
    os: "windows",
    arch: "x86_64",
    runner: windowsX86Runner,
  },
  windowsX86Xl: {
    os: "windows",
    arch: "x86_64",
    runner: isDenoland.then(windowsX86XlRunner).else(windowsX86Runner),
    testRunner: windowsX86Runner,
  },
  windowsArm: {
    os: "windows",
    arch: "aarch64",
    runner: windowsArmRunner,
  },
} as const;

// discover test crates first so we know which workspace members are test packages
const { testCrates, testPackageMembers } = resolveTestCrateTests();
// discover workspace members for the libs test job, split by type
const { binCrates, libCrates } = resolveWorkspaceCrates(
  testPackageMembers,
);

// Note that you may need to add more version to the `apt-get remove` line below if you change this
const llvmVersion = 21;
const installPkgsCommand =
  `sudo apt-get install -y --no-install-recommends clang-${llvmVersion} lld-${llvmVersion} clang-tools-${llvmVersion} clang-format-${llvmVersion} clang-tidy-${llvmVersion}`;
const sysRootConfig = {
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
${installPkgsCommand} || (echo 'Failed. Trying again.' && sudo apt-get clean && sudo apt-get update && ${installPkgsCommand})
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

function handleBuildItems(items: {
  skip_pr?: Condition | true;
  skip?: Condition | boolean;
  os: "linux" | "macos" | "windows";
  arch: "x86_64" | "aarch64";
  runner: string | ExpressionValue;
  profile: string;
  use_sysroot?: boolean;
  testRunner?: string | ExpressionValue;
  wpt?: Condition | boolean;
}[]) {
  return items.map(({ skip_pr, ...rest }) => {
    const defaultValues = {
      skip: false,
      "use_sysroot": false,
      wpt: false,
    };
    if (skip_pr == null) {
      return {
        ...defaultValues,
        ...rest,
        save_cache: true,
      };
    } else {
      // on PRs without the ci-full label, use a free runner and skip the job
      const shouldSkip = hasCiFullLabel.not().and(isPr).and(skip_pr);
      return {
        ...defaultValues,
        ...rest,
        testRunner: shouldSkip.then(ubuntuX86Runner).else(
          rest.testRunner ?? rest.runner,
        ),
        runner: shouldSkip.then(ubuntuX86Runner).else(rest.runner),
        skip: shouldSkip,
        // do not save the cache on main if it won't be used by prs most of the time
        save_cache: skip_pr !== true,
      };
    }
  });
}

// shared steps
const cloneRepoStep = step({
  name: "Configure git",
  run: [
    "git config --global core.symlinks true",
    "git config --global fetch.parallel 32",
  ],
}, {
  name: "Clone repository",
  uses: "actions/checkout@v6",
  with: {
    // Use depth > 1, because sometimes we need to rebuild main and if
    // other commits have landed it will become impossible to rebuild if
    // the checkout is too shallow.
    "fetch-depth": 5,
    submodules: false,
  },
});
const cloneSubmodule = (path: string) =>
  step({
    name: `Clone submodule ${path}`,
    run: `git submodule update --init --recursive --depth=1 -- ${path}`,
  });
const cloneStdSubmoduleStep = cloneSubmodule("./tests/util/std");
const installDenoStep = step({
  name: "Install Deno",
  uses: "denoland/setup-deno@v2",
  with: { "deno-version": "v2.x" },
});
const installNodeStep = step({
  name: "Install Node",
  uses: "actions/setup-node@v6",
  with: {
    "node-version": 22,
  },
});

function createRestoreAndSaveCacheSteps(m: {
  name: string;
  cacheKeyPrefix: string;
  path: string[];
}) {
  // this must match for save and restore (https://github.com/actions/cache/issues/1444)
  const path = m.path.join("\n");
  const restoreCacheStep = step({
    name: `Restore cache ${m.name}`,
    uses: "cirruslabs/cache/restore@v4",
    with: {
      path,
      key: "never_saved",
      "restore-keys": `${m.cacheKeyPrefix}-`,
    },
  });
  const saveCacheStep = step({
    name: `Cache ${m.name}`,
    uses: "cirruslabs/cache/save@v4",
    with: {
      path,
      // We force saving a new cache on every main run so that PRs can
      // always be up to date with the freshest information. We do this
      // unconditionally because we don't want caches that only need updating
      // occassionally (like the cargo home cache) to be lost over time as
      // other caches that need to be updated frequently (like the cargo build
      // cache) get populated and purge old caches.
      key: `${m.cacheKeyPrefix}-\${{ github.sha }}`,
    },
  });
  return { restoreCacheStep, saveCacheStep };
}

function createCargoCacheHomeStep(m: {
  os: ExpressionValue;
  arch: ExpressionValue;
  cachePrefix: string;
}) {
  const steps = createRestoreAndSaveCacheSteps({
    name: "cargo home",
    path: [
      "~/.cargo/.crates.toml",
      "~/.cargo/.crates2.json",
      "~/.cargo/bin",
      "~/.cargo/registry/index",
      "~/.cargo/registry/cache",
      "~/.cargo/git/db",
    ],
    cacheKeyPrefix:
      `${cacheVersion}-cargo-home-${m.os}-${m.arch}-${m.cachePrefix}`,
  });

  return {
    restoreCacheStep: steps.restoreCacheStep.if(isNotTag),
    saveCacheStep: steps.saveCacheStep.if(isMainBranch.and(isNotTag)),
  };
}

// factory for cache steps parameterized by os/arch/profile/job
// works with both defineExprObj (inline values) and defineMatrix (matrix expressions)
function createCacheSteps(m: {
  os: ExpressionValue;
  arch: ExpressionValue;
  profile: ExpressionValue;
  cachePrefix: string;
}) {
  const cargoHomeCacheSteps = createCargoCacheHomeStep(m);
  const buildCacheSteps = createRestoreAndSaveCacheSteps({
    name: "build output",
    path: [
      "./target",
      "!./target/*/gn_out",
      "!./target/*/gn_root",
      "!./target/*/*.zip",
      "!./target/*/*.tar.gz",
    ],
    cacheKeyPrefix:
      `${cacheVersion}-cargo-target-${m.os}-${m.arch}-${m.profile}-${m.cachePrefix}`,
  });
  const mtimeCacheAndRestoreStep = step({
    name: "Apply and update mtime cache",
    uses: "./.github/mtime_cache",
    with: {
      "cache-path": "./target",
    },
  });
  return {
    restoreCacheStep: step(
      cargoHomeCacheSteps.restoreCacheStep,
      buildCacheSteps.restoreCacheStep.if(isMainBranch.not().and(isNotTag)),
      // this should always be done when saving OR restoring
      mtimeCacheAndRestoreStep,
    ),
    saveCacheStep: step(
      cargoHomeCacheSteps.saveCacheStep,
      // todo(THIS PR): commented out to test (1)
      // buildCacheSteps.saveCacheStep.if(isMainBranch.and(isNotTag)),
      buildCacheSteps.saveCacheStep,
    ),
  };
}
const installRustStep = step({
  uses: "dsherret/rust-toolchain-file@v1",
});
const installWasmStep = step({
  name: "Install wasm target",
  run: "rustup target add wasm32-unknown-unknown",
});

function getOsSpecificSteps({
  isWindows,
  isMacos,
  isAarch64,
}: {
  isWindows: Condition;
  isMacos: Condition;
  isAarch64: Condition;
}) {
  const installPythonStep = step({
    name: "Install Python",
    uses: "actions/setup-python@v6",
    with: {
      "python-version": 3.11,
    },
  }, {
    name: "Remove unused versions of Python",
    if: isWindows,
    shell: "pwsh",
    run: [
      '$env:PATH -split ";" |',
      '  Where-Object { Test-Path "$_\\python.exe" } |',
      "  Select-Object -Skip 1 |",
      '  ForEach-Object { Move-Item "$_" "$_.disabled" }',
    ],
  });
  const setupPrebuiltMacStep = step({
    if: isMacos,
    env: {
      GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}",
    },
    run: "echo $GITHUB_WORKSPACE/third_party/prebuilt/mac >> $GITHUB_PATH",
  });
  const installLldStep = step
    .dependsOn(
      cloneStdSubmoduleStep,
      installDenoStep,
      setupPrebuiltMacStep,
    )({
      name: "Install macOS aarch64 lld",
      if: isMacos.and(isAarch64),
      env: {
        GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}",
      },
      run: "./tools/install_prebuilt.js ld64.lld",
    });
  const setupGcloudStep = step(
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
      name: "Setup gcloud (unix)",
      if: isWindows.not(),
      uses: "google-github-actions/setup-gcloud@v3",
      with: { project_id: "denoland" },
    },
    step({
      name: "Setup gcloud (windows)",
      if: isWindows,
      uses: "google-github-actions/setup-gcloud@v2",
      env: { CLOUDSDK_PYTHON: "${{env.pythonLocation}}\\python.exe" },
      with: { project_id: "denoland" },
    }).dependsOn(installPythonStep),
  );
  return {
    installPythonStep,
    setupPrebuiltMacStep,
    installLldStep,
    setupGcloudStep,
  };
}

// === pre_build job ===
// The pre_build step is used to skip running the CI on draft PRs and to not even
// start the build job. This can be overridden by adding [ci] to the commit title

const preBuildCheckStep = step({
  id: "check",
  if: conditions.hasPrLabel("ci-draft").not(),
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
  steps: step.if(conditions.isDraftPr())(
    cloneRepoStep,
    preBuildCheckStep,
  ),
  outputs: {
    skip_build: preBuildCheckStep.outputs.skip_build,
  },
});

// === build job ===

const buildItems = handleBuildItems([{
  ...Runners.macosX86,
  profile: "debug",
}, {
  ...Runners.macosX86,
  profile: "release",
  skip_pr: true,
}, {
  ...Runners.macosArm,
  profile: "debug",
}, {
  ...Runners.macosArmSelfHosted,
  profile: "release",
  skip_pr: true,
}, {
  ...Runners.windowsX86,
  profile: "debug",
}, {
  ...Runners.windowsX86Xl,
  profile: "release",
  skip_pr: true,
}, {
  ...Runners.windowsArm,
  profile: "debug",
}, {
  ...Runners.windowsArm,
  profile: "release",
  skip_pr: true,
}, {
  ...Runners.linuxX86Xl,
  profile: "release",
  use_sysroot: true,
  // Because CI is so slow on for OSX and Windows, we
  // currently run the Web Platform tests only on Linux.
  wpt: isNotTag,
}, {
  ...Runners.linuxX86,
  profile: "debug",
  use_sysroot: true,
}, {
  ...Runners.linuxArm,
  profile: "debug",
}, {
  ...Runners.linuxArm,
  profile: "release",
  use_sysroot: true,
  skip_pr: true,
}]);

const buildJobs = buildItems.map((rawBuildItem) => {
  const buildItem = defineExprObj(rawBuildItem);
  const isLinux = buildItem.os.equals("linux");
  const isWindows = buildItem.os.equals("windows");
  const isMacos = buildItem.os.equals("macos");
  const profileName = `${buildItem.profile}-${buildItem.os}-${buildItem.arch}`;
  const jobIdForJob = (name: string) => `${name}-${profileName}`;
  const jobNameForJob = (name: string) =>
    `${name} ${buildItem.profile} ${buildItem.os}-${buildItem.arch}`;
  const createBinaryArtifact = (name: string) => {
    const directory = `target/${buildItem.profile}`;
    const exeExt = rawBuildItem.os === "windows" ? ".exe" : "";
    const fileName = `${name}${exeExt}`;
    const artifact = defineArtifact(
      `${profileName}-${name.replaceAll("_", "-")}`,
      {
        retentionDays: 3,
      },
    );
    const filePath = `${directory}/${fileName}`;
    return {
      upload() {
        return artifact.upload({
          path: filePath,
        });
      },
      download() {
        return step(
          artifact.download({
            dirPath: directory,
          }),
          step({
            name: `Set ${filePath} permissions`,
            if: isWindows.not(),
            run: `chmod +x ${filePath}`,
          }),
        );
      },
    };
  };

  const denoArtifact = createBinaryArtifact("deno");
  const denortArtifact = createBinaryArtifact("denort");
  const testServerArtifact = createBinaryArtifact("test_server");
  const env = {
    CARGO_TERM_COLOR: "always",
    RUST_BACKTRACE: "full",
    // disable anyhow's library backtrace
    RUST_LIB_BACKTRACE: 0,
  };
  const defaults = {
    run: {
      // GH actions does not fail fast by default on
      // Windows, so we set bash as the default shell
      shell: "bash",
    },
  };

  const {
    installPythonStep,
    setupPrebuiltMacStep,
    installLldStep,
    setupGcloudStep,
  } = getOsSpecificSteps({
    isWindows,
    isMacos,
    isAarch64: buildItem.arch.equals("aarch64"),
  });
  const isRelease = buildItem.profile.equals("release");
  const isDebug = buildItem.profile.equals("debug");
  const sysRootStep = step({
    if: buildItem.use_sysroot,
    ...sysRootConfig,
  });
  const buildJob = job(
    jobIdForJob("build"),
    {
      name: jobNameForJob("build"),
      needs: [preBuildJob],
      if: preBuildJob.outputs.skip_build.notEquals("true"),
      runsOn: buildItem.runner,
      // This is required to successfully authenticate with Azure using OIDC for
      // code signing.
      environment: {
        name: isMainOrTag.then("build").else(""),
      },
      timeoutMinutes: 240,
      defaults,
      env,
      steps: (() => {
        const {
          restoreCacheStep,
          saveCacheStep,
        } = createCacheSteps({
          ...buildItem,
          cachePrefix: "build-main",
        });
        const tarSourcePublishStep = step({
          name: "Create source tarballs (release, linux)",
          if: buildItem.os.equals("linux")
            .and(buildItem.arch.equals("x86_64")),
          run: [
            "mkdir -p target/release",
            'tar --exclude=".git*" --exclude=target --exclude=third_party/prebuilt \\',
            "    -czvf target/release/deno_src.tar.gz -C .. deno",
          ],
        });

        const preRelease = step(
          {
            name: "Pre-release (linux)",
            if: isLinux.and(isDenoland),
            run: [
              "cd target/release",
              `./deno -A ../../tools/release/create_symcache.ts deno-${buildItem.arch}-unknown-linux-gnu.symcache`,
              "strip ./deno",
              `zip -r deno-${buildItem.arch}-unknown-linux-gnu.zip deno`,
              `shasum -a 256 deno-${buildItem.arch}-unknown-linux-gnu.zip > deno-${buildItem.arch}-unknown-linux-gnu.zip.sha256sum`,
              "strip ./denort",
              `zip -r denort-${buildItem.arch}-unknown-linux-gnu.zip denort`,
              `shasum -a 256 denort-${buildItem.arch}-unknown-linux-gnu.zip > denort-${buildItem.arch}-unknown-linux-gnu.zip.sha256sum`,
              "./deno types > lib.deno.d.ts",
            ],
          },
          step.dependsOn(setupPrebuiltMacStep, installDenoStep)({
            name: "Install rust-codesign",
            if: buildItem.os.equals("macos").and(isDenoland),
            env: {
              GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}",
            },
            run: "./tools/install_prebuilt.js rcodesign",
          }),
          {
            name: "Pre-release (mac)",
            if: isMacos.and(isDenoland),
            env: {
              "APPLE_CODESIGN_KEY": "${{ secrets.APPLE_CODESIGN_KEY }}",
              "APPLE_CODESIGN_PASSWORD":
                "${{ secrets.APPLE_CODESIGN_PASSWORD }}",
            },
            run: [
              `target/release/deno -A tools/release/create_symcache.ts target/release/deno-${buildItem.arch}-apple-darwin.symcache`,
              "strip -x -S target/release/deno",
              'echo "Key is $(echo $APPLE_CODESIGN_KEY | base64 -d | wc -c) bytes"',
              "rcodesign sign target/release/deno " +
              "--code-signature-flags=runtime " +
              '--p12-password="$APPLE_CODESIGN_PASSWORD" ' +
              "--p12-file=<(echo $APPLE_CODESIGN_KEY | base64 -d) " +
              "--entitlements-xml-file=cli/entitlements.plist",
              "cd target/release",
              `zip -r deno-${buildItem.arch}-apple-darwin.zip deno`,
              `shasum -a 256 deno-${buildItem.arch}-apple-darwin.zip > deno-${buildItem.arch}-apple-darwin.zip.sha256sum`,
              "strip -x -S ./denort",
              `zip -r denort-${buildItem.arch}-apple-darwin.zip denort`,
              `shasum -a 256 denort-${buildItem.arch}-apple-darwin.zip > denort-${buildItem.arch}-apple-darwin.zip.sha256sum`,
            ],
          },
          {
            // Note: Azure OIDC credentials are only valid for 5 minutes, so
            // authentication must be done right before signing.
            name: "Authenticate with Azure (windows)",
            if: isWindows.and(isDenoland).and(isMainOrTag),
            uses: "azure/login@v2",
            with: {
              "client-id": "${{ secrets.AZURE_CLIENT_ID }}",
              "tenant-id": "${{ secrets.AZURE_TENANT_ID }}",
              "subscription-id": "${{ secrets.AZURE_SUBSCRIPTION_ID }}",
              "enable-AzPSSession": true,
            },
          },
          {
            name: "Code sign deno.exe (windows)",
            if: isWindows.and(isDenoland).and(isMainOrTag),
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
            if: isWindows.and(isDenoland).and(isMainOrTag),
            shell: "pwsh",
            run: [
              '$SignTool = Get-ChildItem -Path "C:\\Program Files*\\Windows Kits\\*\\bin\\*\\x64\\signtool.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1',
              "$SignToolPath = $SignTool.FullName",
              "& $SignToolPath verify /pa /v target\\release\\deno.exe",
            ],
          },
          {
            name: "Pre-release (windows)",
            if: isWindows.and(isDenoland),
            shell: "pwsh",
            run: [
              `Compress-Archive -CompressionLevel Optimal -Force -Path target/release/deno.exe -DestinationPath target/release/deno-${buildItem.arch}-pc-windows-msvc.zip`,
              `Get-FileHash target/release/deno-${buildItem.arch}-pc-windows-msvc.zip -Algorithm SHA256 | Format-List > target/release/deno-${buildItem.arch}-pc-windows-msvc.zip.sha256sum`,
              `Compress-Archive -CompressionLevel Optimal -Force -Path target/release/denort.exe -DestinationPath target/release/denort-${buildItem.arch}-pc-windows-msvc.zip`,
              `Get-FileHash target/release/denort-${buildItem.arch}-pc-windows-msvc.zip -Algorithm SHA256 | Format-List > target/release/denort-${buildItem.arch}-pc-windows-msvc.zip.sha256sum`,
              `target/release/deno.exe -A tools/release/create_symcache.ts target/release/deno-${buildItem.arch}-pc-windows-msvc.symcache`,
            ],
          },
          step.dependsOn(setupGcloudStep)({
            name: "Upload canary to dl.deno.land",
            if: isDenoland.and(isMainBranch),
            run: [
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/canary/$(git rev-parse HEAD)/',
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.sha256sum gs://dl.deno.land/canary/$(git rev-parse HEAD)/',
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.symcache gs://dl.deno.land/canary/$(git rev-parse HEAD)/',
              "echo ${{ github.sha }} > canary-latest.txt",
              'gsutil -h "Cache-Control: no-cache" cp canary-latest.txt gs://dl.deno.land/canary-$(rustc -vV | sed -n "s|host: ||p")-latest.txt',
              "rm canary-latest.txt gha-creds-*.json",
            ],
          }),
        );
        const binsToBuild = ["deno", "denort", "test_server"]
          .map((name) => `--bin ${name}`).join(" ");
        const cargoBuildReleaseStep = step
          .if(
            isRelease.and(isDenoland.or(buildItem.use_sysroot)),
          )
          .dependsOn(
            installLldStep,
            restoreCacheStep,
            installRustStep,
            sysRootStep,
          )(
            {
              // do this on PRs as well as main so that PRs can use the cargo build cache from main
              name: "Configure canary build",
              run: 'echo "DENO_CANARY=true" >> $GITHUB_ENV',
            },
            {
              name: "Build release",
              run: [
                // output fs space before and after building
                "df -h",
                `cargo build --release --locked ${binsToBuild} --features=panic-trace`,
                "df -h",
              ],
            },
            {
              name: "Generate symcache",
              run: [
                "target/release/deno -A tools/release/create_symcache.ts ./deno.symcache",
                "du -h deno.symcache",
                "du -h target/release/deno",
              ],
              env: { NO_COLOR: 1 },
            },
            preRelease,
            {
              name: "Build product size info",
              if: isMainOrTag,
              run: [
                `du -hd1 "./target/${buildItem.profile}"`,
                `du -ha  "./target/${buildItem.profile}/deno"`,
                `du -ha  "./target/${buildItem.profile}/denort"`,
              ],
            },
          );
        const cargoBuildStep = step
          .dependsOn(
            installLldStep,
            restoreCacheStep,
            installRustStep,
            sysRootStep,
          )
          .comesAfter(tarSourcePublishStep)(
            {
              name: "Build debug",
              if: isDebug,
              run: `cargo build --locked ${binsToBuild} --features=panic-trace`,
              env: { CARGO_PROFILE_DEV_DEBUG: 0 },
            },
            cargoBuildReleaseStep,
            {
              // Run a minimal check to ensure that binary is not corrupted, regardless
              // of our build mode
              name: "Check deno binary",
              run:
                `target/${buildItem.profile}/deno eval "console.log(1+2)" | grep 3`,
              env: { NO_COLOR: 1 },
            },
            {
              // Verify that the binary actually works in the Ubuntu-16.04 sysroot.
              name: "Check deno binary (in sysroot)",
              if: buildItem.use_sysroot,
              run:
                `sudo chroot /sysroot "$(pwd)/target/${buildItem.profile}/deno" --version`,
            },
            denoArtifact.upload(),
            denortArtifact.upload(),
            testServerArtifact.upload(),
          );

        const shouldPublishCondition = isRelease.and(isDenoland)
          .and(isTag);
        const publishStep = step.if(shouldPublishCondition)(
          step.dependsOn(setupGcloudStep)({
            name: "Upload release to dl.deno.land (unix)",
            if: isWindows.not(),
            run: [
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.sha256sum gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.symcache gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
            ],
          }, {
            name: "Upload release to dl.deno.land (windows)",
            if: isWindows,
            env: { CLOUDSDK_PYTHON: "${{env.pythonLocation}}\\python.exe" },
            run: [
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.sha256sum gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.symcache gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/',
            ],
          }),
          {
            name: "Create release notes",
            run: [
              "export PATH=$PATH:$(pwd)/target/release",
              "./tools/release/05_create_release_notes.ts",
            ],
          },
          {
            name: "Upload release to GitHub",
            uses: "softprops/action-gh-release@v2",
            env: {
              GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}",
            },
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
        );

        return step.if(buildItem.skip.not())(
          cloneRepoStep,
          cloneStdSubmoduleStep,
          // ensure this happens right after cloning
          tarSourcePublishStep.if(shouldPublishCondition),
          {
            name: "Remove macOS cURL --ipv4 flag",
            run: [
              // cURL's --ipv4 flag is busted for now
              "curl --version",
              "which curl",
              "cat /etc/hosts",
              "rm ~/.curlrc || true",
            ],
            if: buildItem.os.equals("macos"),
          },
          step({
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
          }).comesAfter(
            installDenoStep,
            installNodeStep,
            installPythonStep,
            installRustStep,
          ),
          cargoBuildStep,
          publishStep,
          saveCacheStep.if(buildItem.save_cache),
        );
      })(),
    },
  );

  const additionalJobs = [];

  {
    const testMatrix = defineMatrix({
      include: testCrates.map((tc) => ({
        test_crate: tc.name,
        test_package: tc.package,
      })),
    });
    const testCrateNameExpr = testMatrix.test_crate;
    const {
      restoreCacheStep,
      saveCacheStep,
    } = createCacheSteps({
      ...buildItem,
      cachePrefix: `test-${testCrateNameExpr}`,
    });
    additionalJobs.push(job(
      jobIdForJob("test"),
      {
        name:
          `test ${testMatrix.test_crate} ${buildItem.profile} ${buildItem.os}-${buildItem.arch}`,
        needs: [buildJob],
        runsOn: buildItem.testRunner ?? buildItem.runner,
        timeoutMinutes: 240,
        defaults,
        env,
        strategy: {
          matrix: testMatrix,
          failFast: false,
        },
        steps: step.if(isNotTag.and(buildItem.skip.not()))(
          cloneRepoStep,
          cloneSubmodule("./tests/node_compat/runner/suite")
            .if(testCrateNameExpr.equals("node_compat")),
          cloneStdSubmoduleStep,
          restoreCacheStep,
          installNodeStep,
          installRustStep,
          installLldStep,
          sysRootStep,
          denoArtifact.download(),
          denortArtifact.download().if(
            testCrateNameExpr.equals("integration")
              .or(testCrateNameExpr.equals("specs")),
          ),
          testServerArtifact.download().if(
            testCrateNameExpr.equals("integration")
              .or(testCrateNameExpr.equals("specs"))
              .or(testCrateNameExpr.equals("unit"))
              .or(testCrateNameExpr.equals("unit_node")),
          ),
          {
            name: "Set up playwright cache",
            uses: "actions/cache@v5",
            with: {
              path: "./.ms-playwright",
              key: "playwright-${{ runner.os }}-${{ runner.arch }}",
            },
          },
          {
            if: buildItem.os.equals("linux").and(
              buildItem.arch.equals("aarch64"),
            ),
            name: "Load 'vsock_loopback; kernel module",
            run: "sudo modprobe vsock_loopback",
          },
          {
            name: "Build ffi (debug)",
            if: isDebug.and(testCrateNameExpr.equals("specs")),
            run: "cargo build -p test_ffi",
          },
          {
            name: "Build ffi (release)",
            if: isRelease.and(testCrateNameExpr.equals("specs")),
            run: "cargo build --release -p test_ffi",
          },
          {
            name: "Test (debug)",
            if: isDebug,
            run:
              `cargo test -p ${testMatrix.test_package} --test ${testMatrix.test_crate}`,
            env: { CARGO_PROFILE_DEV_DEBUG: 0 },
          },
          {
            name: "Test (release)",
            if: isRelease.and(
              isDenoland.or(buildItem.use_sysroot),
            ),
            run:
              `cargo test -p ${testMatrix.test_package} --test ${testMatrix.test_crate} --release`,
          },
          {
            name: "Ensure no git changes",
            if: isPr,
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
          step.dependsOn(installDenoStep)({
            name: "Upload test results",
            uses: "actions/upload-artifact@v6",
            if: conditions.status.always().and(isNotTag),
            with: {
              name:
                `test-results-${buildItem.os}-${buildItem.arch}-${buildItem.profile}-${testMatrix.test_crate}.json`,
              path: `target/test_results_${testMatrix.test_crate}.json`,
            },
          }),
          saveCacheStep.if(buildItem.save_cache),
        ),
      },
    ));
  }

  const libsCondition = isDebug.and(
    // aarc64 runner seems faster than x86
    isLinux.and(buildItem.arch.equals("aarch64"))
      .or(isMacos.and(buildItem.arch.equals("aarch64")))
      .or(isWindows.and(buildItem.arch.equals("x86_64"))),
  );
  if (libsCondition.isPossiblyTrue()) {
    const {
      restoreCacheStep,
      saveCacheStep,
    } = createCacheSteps({
      ...buildItem,
      cachePrefix: "test-libs",
    });
    additionalJobs.push(job(jobIdForJob("test-libs"), {
      name: jobNameForJob("test libs"),
      needs: [buildJob],
      runsOn: buildItem.testRunner ?? buildItem.runner,
      timeoutMinutes: 30,
      steps: step.if(isNotTag.and(buildItem.skip.not()))(
        cloneRepoStep,
        restoreCacheStep,
        installNodeStep,
        installRustStep,
        installLldStep,
        sysRootStep,
        denoArtifact.download(),
        testServerArtifact.download(),
        {
          name: "Test libs",
          run: `cargo test --locked --lib ${
            [...binCrates, ...libCrates].map((p) => `-p ${p}`).join(" ")
          }`,
          env: { CARGO_PROFILE_DEV_DEBUG: 0 },
        },
        saveCacheStep,
      ),
    }));
  }
  if (
    isDebug.and(isLinux).and(buildItem.arch.equals("x86_64")).isPossiblyTrue()
  ) {
    const {
      restoreCacheStep,
      saveCacheStep,
    } = createCacheSteps({
      ...buildItem,
      cachePrefix: "build-libs",
    });
    additionalJobs.push(job(jobIdForJob("build-libs"), {
      name: jobNameForJob("build libs"),
      needs: [preBuildJob],
      if: preBuildJob.outputs.skip_build.notEquals("true"),
      runsOn: buildItem.runner,
      timeoutMinutes: 30,
      steps: step.if(isNotTag.and(buildItem.skip.not()))(
        cloneRepoStep,
        installRustStep,
        restoreCacheStep,
        installWasmStep,
        // we want these crates to be Wasm compatible
        {
          name: "Cargo check (deno_resolver)",
          run:
            "cargo check --target wasm32-unknown-unknown -p deno_resolver && cargo check --target wasm32-unknown-unknown -p deno_resolver --features graph && cargo check --target wasm32-unknown-unknown -p deno_resolver --features graph --features deno_ast",
        },
        {
          name: "Cargo check (deno_npm_installer)",
          run:
            "cargo check --target wasm32-unknown-unknown -p deno_npm_installer",
        },
        {
          name: "Cargo check (deno_config)",
          run: [
            "cargo check --no-default-features -p deno_config",
            "cargo check --no-default-features --features workspace -p deno_config",
            "cargo check --no-default-features --features package_json -p deno_config",
            "cargo check --no-default-features --features workspace --features sync -p deno_config",
            "cargo check --target wasm32-unknown-unknown --all-features -p deno_config",
            "cargo check -p deno --features=lsp-tracing",
          ],
        },
        saveCacheStep,
      ),
    }));
  }

  if (buildItem.wpt.isPossiblyTrue()) {
    additionalJobs.push(job(
      jobIdForJob("wpt"),
      {
        name: jobNameForJob("wpt"),
        needs: [buildJob],
        runsOn: buildItem.testRunner ?? buildItem.runner,
        timeoutMinutes: 240,
        defaults,
        env,
        steps: step.if(isNotTag.and(buildItem.skip.not()))(
          cloneRepoStep,
          cloneStdSubmoduleStep,
          cloneSubmodule("./tests/wpt/suite"),
          installDenoStep,
          installPythonStep,
          denoArtifact.download(),
          {
            name: "Configure hosts file for WPT",
            run: "./wpt make-hosts-file | sudo tee -a /etc/hosts",
            workingDirectory: "tests/wpt/suite/",
          },
          {
            name: "Run web platform tests (debug)",
            if: isDebug,
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
            if: isRelease,
            env: {
              DENO_BIN: "./target/release/deno",
            },
            run: [
              "deno run -RWNE --allow-run --lock=tools/deno.lock.json --config tests/config/deno.json \\",
              "    ./tests/wpt/wpt.ts setup",
              "deno run -RWNE --allow-run --lock=tools/deno.lock.json --config tests/config/deno.json --unsafely-ignore-certificate-errors \\",
              '    ./tests/wpt/wpt.ts run --quiet --release --binary="$DENO_BIN" --json=wpt.json --wptreport=wptreport.json',
            ],
          },
          {
            name: "Autobahn testsuite",
            if: isRelease,
            run:
              "target/release/deno run -A --config tests/config/deno.json ext/websocket/autobahn/fuzzingclient.js",
          },
          step.dependsOn(setupGcloudStep)({
            name: "Upload wpt results to dl.deno.land",
            continueOnError: true,
            if: isRelease.and(isLinux).and(isDenoland).and(isMainBranch).and(
              isNotTag,
            ),
            run: [
              "gzip ./wptreport.json",
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./wpt.json gs://dl.deno.land/wpt/$(git rev-parse HEAD).json',
              'gsutil -h "Cache-Control: public, max-age=3600" cp ./wptreport.json.gz gs://dl.deno.land/wpt/$(git rev-parse HEAD)-wptreport.json.gz',
              "echo $(git rev-parse HEAD) > wpt-latest.txt",
              'gsutil -h "Cache-Control: no-cache" cp wpt-latest.txt gs://dl.deno.land/wpt-latest.txt',
            ],
          }),
          {
            name: "Upload wpt results to wpt.fyi",
            continueOnError: true,
            if: isRelease.and(isLinux).and(isDenoland).and(isMainBranch).and(
              isNotTag,
            ),
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
        ),
      },
    ));
  }

  return {
    buildJob,
    additionalJobs,
  };
});

// === bench job ===

const benchProfile = defineExprObj(Runners.linuxX86Xl);
const benchCacheSteps = createCargoCacheHomeStep({
  ...benchProfile,
  cachePrefix: "bench",
});
const benchJob = job(
  "bench",
  {
    name: `bench release ${benchProfile.os}-${benchProfile.arch}`,
    needs: [preBuildJob],
    if: preBuildJob.outputs.skip_build.notEquals("true"),
    runsOn: benchProfile.runner,
    timeoutMinutes: 240,
    defaults: {
      run: {
        // GH actions does not fail fast by default on
        // Windows, so we set bash as the default shell
        shell: "bash",
      },
    },
    steps: step
      .if(
        (hasCiBenchLabel.or(isMainBranch)).and(isNotTag),
      )(
        cloneRepoStep,
        benchCacheSteps.restoreCacheStep,
        installNodeStep,
        installRustStep,
        cloneSubmodule("./tests/bench/testdata/lsp_benchdata"),
        cloneStdSubmoduleStep,
        step(sysRootConfig),
        installDenoStep,
        {
          name: "Install benchmark tools",
          env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" },
          run: "./tools/install_prebuilt.js wrk hyperfine",
        },
        // We currently do a full deno build instead of getting this from the build
        // job because the benchmarks inspect the target folder to see the sizes of
        // libraries like v8 and swc as well as the snapshot sizes. Maybe in the future
        // we could optimize this to not need this.
        {
          name: "Build deno",
          run: "cargo build --release -p deno",
        },
        {
          name: "Run benchmarks",
          run: "cargo bench -p bench_tests --bench deno_bench --locked",
        },
        {
          name: "Post benchmarks",
          if: isDenoland.and(isMainBranch),
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
          ],
        },
        {
          name: "Worker info",
          run: ["cat /proc/cpuinfo", "cat /proc/meminfo"],
        },
        benchCacheSteps.saveCacheStep,
      ),
  },
);

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
  defaults: {
    run: {
      shell: "bash",
    },
  },
  strategy: {
    matrix: lintMatrix,
  },
  steps: (() => {
    const {
      restoreCacheStep,
      saveCacheStep,
    } = createCacheSteps({
      ...lintMatrix,
      cachePrefix: "lint",
    });
    return step(
      cloneRepoStep,
      cloneStdSubmoduleStep,
      restoreCacheStep,
      installRustStep,
      installDenoStep,
      step.if(lintMatrix.os.equals("linux"))(
        {
          name: "test_format.js",
          run:
            "deno run --allow-write --allow-read --allow-run --allow-net ./tools/format.js --check",
        },
        {
          name: "jsdoc_checker.js",
          run:
            "deno run --allow-read --allow-env --allow-sys ./tools/jsdoc_checker.js",
        },
      ),
      {
        name: "lint.js",
        env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" },
        run:
          "deno run --allow-write --allow-read --allow-run --allow-net --allow-env ./tools/lint.js",
      },
      saveCacheStep,
    );
  })(),
});

// === publish-canary job ===

const publishCanaryJob = job("publish-canary", {
  name: "publish canary",
  runsOn: ubuntuX86Runner,
  needs: [...buildJobs.map((b) => b.buildJob)],
  if: isDenoland.and(isMainBranch),
  steps: (() => {
    const {
      setupGcloudStep,
    } = getOsSpecificSteps({
      // we only run this on linux
      isWindows: conditions.isFalse(),
      isMacos: conditions.isFalse(),
      isAarch64: conditions.isFalse(),
    });
    return step(
      setupGcloudStep,
      {
        name: "Upload canary version file to dl.deno.land",
        run: [
          "echo ${{ github.sha }} > canary-latest.txt",
          'gsutil -h "Cache-Control: no-cache" cp canary-latest.txt gs://dl.deno.land/canary-latest.txt',
        ],
      },
    );
  })(),
});

// === lint ci status job (status check gate) ===

const lintCiStatusJob = job("lint-ci-status", {
  name: "lint ci status",
  // We use this job in the main branch rule status checks for PRs.
  // All jobs that are required to pass on a PR should be listed here.
  needs: [
    benchJob,
    ...buildJobs.map((j) => [j.buildJob, ...j.additionalJobs]).flat(),
    lintJob,
  ],
  if: preBuildJob.outputs.skip_build.notEquals("true")
    .and(conditions.status.always()),
  runsOn: "ubuntu-latest",
  steps: step({
    name: "Ensure CI success",
    run: [
      "if [[ \"${{ contains(needs.*.result, 'failure') || contains(needs.*.result, 'cancelled') }}\" == \"true\" ]]; then",
      "  echo 'CI failed'",
      "  exit 1",
      "fi",
    ],
  }),
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
  jobs: [
    preBuildJob,
    benchJob,
    ...buildJobs.map((j) => [j.buildJob, ...j.additionalJobs]).flat(),
    lintJob,
    lintCiStatusJob,
    publishCanaryJob,
  ],
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

function resolveTestCrateTests() {
  const rootCargoToml = parseToml(
    Deno.readTextFileSync(new URL("../../Cargo.toml", import.meta.url)),
  ) as { workspace: { members: string[] } };

  const testCrates: { name: string; package: string }[] = [];
  const testPackageMembers = new Set<string>();

  for (const member of rootCargoToml.workspace.members) {
    if (!member.startsWith("tests")) continue;
    const cargoToml = parseToml(
      Deno.readTextFileSync(
        new URL(`../../${member}/Cargo.toml`, import.meta.url),
      ),
    ) as {
      package: { name: string; autotests?: boolean };
      test?: { name: string; path: string }[];
    };
    // only include crates that explicitly disable auto-test discovery,
    // indicating they are intentional test packages (not helper libraries
    // like tests/ffi or tests/util/server)
    if (cargoToml.package.autotests !== false) continue;
    const tests = cargoToml.test ?? [];
    if (tests.length > 0) {
      testPackageMembers.add(member);
      for (const test of tests) {
        testCrates.push({ name: test.name, package: cargoToml.package.name });
      }
    }
  }

  return { testCrates, testPackageMembers };
}

function resolveWorkspaceCrates(testPackageMembers: Set<string>) {
  // discover workspace members for the libs test job, split by type
  const rootCargoToml = parseToml(
    Deno.readTextFileSync(new URL("../../Cargo.toml", import.meta.url)),
  ) as { workspace: { members: string[] } };

  const libCrates: string[] = [];
  const binCrates: string[] = [];
  for (const member of rootCargoToml.workspace.members) {
    const cargoToml = parseToml(
      Deno.readTextFileSync(
        new URL(`../../${member}/Cargo.toml`, import.meta.url),
      ),
    ) as {
      package: { name: string };
      bin?: unknown[];
      test?: { path?: string }[];
    };

    if (member.startsWith("tests")) {
      if (!testPackageMembers.has(member)) {
        ensureNoIntegrationTests(member, cargoToml);
      }
    } else if (cargoToml.bin) {
      ensureNoIntegrationTests(member, cargoToml);
      binCrates.push(cargoToml.package.name);
    } else {
      libCrates.push(cargoToml.package.name);
    }
  }
  return { libCrates, binCrates };
}

function ensureNoIntegrationTests(
  member: string,
  cargoToml: {
    package: { name: string };
    test?: { path?: string }[];
  },
) {
  const errors: string[] = [];
  if (existsSync(new URL(`../../${member}/tests/`, import.meta.url))) {
    errors.push("has a tests/ folder");
  }
  const hasNonRunnerTests = cargoToml.test?.some(
    // this path is allowed because it's only used by deno and denort
    // to cause the deno and denort binaries to be built when running
    // tests, but it doesn't actually run any tests itself
    (t) => t.path !== "integration_tests_runner.rs",
  );
  if (hasNonRunnerTests) {
    errors.push("has a [[test]] section in Cargo.toml");
  }
  if (errors.length > 0) {
    throw new Error(
      `crate "${cargoToml.package.name}" (${member}) ${
        errors.join(" and ")
      }. ` +
        `Integration tests in these crates won't run on CI because we build ` +
        `binaries on one runner then test on another. ` +
        `Move them to spec tests, the test crates in tests/, or use #[cfg(test)] lib tests instead.`,
    );
  }
}

function existsSync(path: string | URL) {
  try {
    Deno.statSync(path);
    return true;
  } catch (e) {
    if (!(e instanceof Deno.errors.NotFound)) throw e;
    return false;
  }
}
