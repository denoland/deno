/**
 * Generates a CI configuration file (.github/workflows/ci.yml).
 */

import * as YAML from "https://deno.land/std@0.137.0/encoding/yaml.ts";
import prettier from "https://unpkg.com/prettier@2.7.1/esm/standalone.mjs";
import prettierYaml from "https://unpkg.com/prettier@2.7.1/esm/parser-yaml.mjs";

import { ARCHIVE_COUNT } from "./_util.js";

const CI = {
  name: "ci",
  on: {
    push: { branches: ["main"], tags: ["v*"] },
    pull_request: { branches: ["main"] },
  },
  concurrency: {
    group:
      "${{ github.workflow }}-${{ !contains(github.event.pull_request.labels.*.name, 'test-flaky-ci') && github.head_ref || github.run_id }}",
    "cancel-in-progress": true,
  },
  env: {
    CARGO_TERM_COLOR: "always",
    RUST_BACKTRACE: "full",
    RUSTFLAGS: "-D warnings",
  },
  jobs: {},
};

const RUNNERS = {
  "linux":
    "${{ github.repository == 'denoland/deno' && 'ubuntu-20.04-xl' || 'ubuntu-20.04' }}",
  "macos": "macos-11",
  "windows":
    "${{ github.repository == 'denoland/deno' && 'windows-2019-xl' || 'windows-2019' }}",
};

const CHECKOUT_STEPS = (submodules = []) => [
  {
    name: "Configure Git",
    run: `git config --global core.symlinks true
git config --global fetch.parallel 32`,
  },
  {
    name: "Clone repository",
    uses: "actions/checkout@v3",
    with: {
      // Use depth > 1, because sometimes we need to rebuild main and if other
      // commits have landed it will become impossible to rebuild if the
      // checkout is too shallow.
      "fetch-depth": 5,
      submodules: false,
    },
  },
  ...submodules.map((submodule) => ({
    name: `Clone submodule ${submodule}`,
    run: `git submodule update --init --recursive --depth=1 -- ${submodule}`,
  })),
];

const RESTORE_BUILD = (buildJobName, platform) => [
  {
    name: "Download artifacts",
    uses: "actions/download-artifact@v3",
    with: {
      name: buildJobName,
    },
  },
  {
    name: "Unpack artifacts",
    run: ARTIFACT_PATHS.map((path) =>
      `${platform === "macos" ? "gtar" : "tar"} -xpf ${path}`
    ).join("\n"),
  },
];

const USE_GNU_TAR = (platform) =>
  platform === "windows"
    ? [
      {
        name: "Use GNU tar",
        shell: "cmd",
        run: `echo C:\\Program Files\\Git\\usr\\bin>>"%GITHUB_PATH%"`,
      },
    ]
    : [];

const INSTALL_RUST = {
  name: "Install Rust",
  run: [
    "cargo --version",
    "rustc --version",
    "rustfmt --version",
    "cargo-fmt --version",
    "cargo-clippy --version",
  ].join("\n"),
};

const SETUP_SYSROOT = (platform) =>
  platform === "linux"
    ? [{
      name: "Set up sysroot",
      run: `
# Avoid running man-db triggers, which sometimes takes several minutes
# to complete.
sudo apt-get remove --purge -y man-db
# Install clang-15, lld-15, and debootstrap.
echo "deb http://apt.llvm.org/focal/ llvm-toolchain-focal-15 main" |
  sudo dd of=/etc/apt/sources.list.d/llvm-toolchain-focal-15.list
curl https://apt.llvm.org/llvm-snapshot.gpg.key |
  gpg --dearmor                                 |
sudo dd of=/etc/apt/trusted.gpg.d/llvm-snapshot.gpg
sudo apt-get update
sudo apt-get install --no-install-recommends debootstrap clang-15 lld-15
# Create ubuntu-16.04 sysroot environment, which is used to avoid
# depending on a very recent version of glibc.
# "libc6-dev" is required for building any C source files.
# "file" and "make" are needed to build libffi-sys.
# "curl" is needed to build rusty_v8.
sudo debootstrap \\
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
    }]
    : [];

const INSTALL_DENO = {
  name: "Install Deno",
  uses: "denoland/setup-deno@v1",
  with: { "deno-version": "v1.28.3" },
};

const INSTALL_PYTHON = {
  name: "Install Python",
  uses: "actions/setup-python@v4",
  with: { "python-version": "3.8" },
};

const CACHE_RUST = (key) => ({
  name: "Cache Rust",
  uses: "Swatinem/rust-cache@v1",
  with: { key },
});

CI.jobs.lint = {
  name: "lint",
  "runs-on": RUNNERS.linux,
  "timeout-minutes": 90,
  steps: [
    ...CHECKOUT_STEPS(["./test_util/std", "./third_party"]),
    INSTALL_DENO,
    INSTALL_RUST,
    {
      name: "Check formatting",
      run:
        "deno run --allow-write --allow-read --allow-run --unstable ./tools/format.js --check",
    },
    CACHE_RUST("debug"),
    {
      name: "Lint",
      run:
        "deno run --allow-write --allow-read --allow-run --unstable ./tools/lint.js",
    },
  ],
};

const PLATFORMS = [
  {
    name: "linux",
    targets: [
      {
        target: "debug",
        on: ["pr", "main"],
        test: 1,
      },
      {
        target: "release",
        on: ["pr", "main"],
        bench: true,
        release: true,
        test: 1,
        wpt: 3,
      },
    ],
  },
  {
    name: "macos",
    targets: [
      {
        target: "debug",
        on: ["pr", "main"],
        test: 3,
      },
      {
        target: "release",
        on: ["main"],
        release: true,
        test: 3,
      },
    ],
  },
  {
    name: "windows",
    targets: [
      {
        target: "debug",
        on: ["pr", "main"],
        test: 3,
      },
      {
        target: "release",
        on: ["main"],
        release: true,
        test: 3,
      },
    ],
  },
];

const MAIN_CONDITION =
  "(github.repository == 'denoland/deno' && (github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/')))";

const ARTIFACT_PATHS = Array.from(
  { length: ARCHIVE_COUNT },
  (_, i) => `artifacts_${i + 1}.tar.gz`,
);

const releases = [];
const tests = [];

for (const platform of PLATFORMS) {
  for (const target of platform.targets) {
    const BUILD_JOB_ID = `build-${target.target}-${platform.name}`;
    const BUILD_JOB_NAME = `build/${platform.name} (${target.target})`;
    const build = CI.jobs[BUILD_JOB_ID] = {
      name: BUILD_JOB_NAME,
      "runs-on": RUNNERS[platform.name],
      "timeout-minutes": 120,
      if: true,
      steps: [
        ...CHECKOUT_STEPS(["./test_util/std"]),
        ...USE_GNU_TAR(platform.name),
        INSTALL_DENO,
        INSTALL_RUST,
        ...SETUP_SYSROOT(platform.name),
        CACHE_RUST(target.target),
        {
          name: "Build",
          shell: "bash",
          run: `mkdir -p target/${target.target}
  cargo build ${
            target.target === "release" ? "--release" : ""
          } --locked --all-targets --tests --message-format=json > target/${target.target}/cargo_build_manifest.json`,
          ...(target.target === "release"
            ? {}
            : { env: { CARGO_PROFILE_DEV_DEBUG: 0 } }),
        },
        {
          name: "Package artifacts",
          run:
            `deno run --allow-read --allow-run --unstable ./tools/ci/package.js ${target.target}`,
        },
        {
          name: "Upload artifacts",
          uses: "actions/upload-artifact@v3",
          with: {
            name: BUILD_JOB_ID,
            path: ARTIFACT_PATHS.join("\n"),
          },
        },
      ],
    };

    if (target.release) {
      releases.push([BUILD_JOB_ID, platform.name]);
    }

    const on = target.on;
    if (on.length === 1 && on.includes("main")) {
      build.if = MAIN_CONDITION;
    } else if (on.length === 2 && on.includes("main") && on.includes("pr")) {
      delete build.if;
    } else {
      throw "invalid 'on' condition";
    }

    const TEST_JOB_ID = `test-${target.target}-${platform.name}`;
    const TEST_JOB_NAME =
      `test/${platform.name} (${target.target}, shard \${{ matrix.shard }})`;
    CI.jobs[TEST_JOB_ID] = {
      name: TEST_JOB_NAME,
      "runs-on": RUNNERS[platform.name],
      "timeout-minutes": 90,
      needs: [BUILD_JOB_ID],
      strategy: {
        matrix: {
          shard: Array.from({ length: target.test }, (_, i) => i + 1),
        },
        "fail-fast": false,
      },
      steps: [
        ...CHECKOUT_STEPS(["./test_util/std", "./third_party"]),
        ...USE_GNU_TAR(platform.name),
        INSTALL_DENO,
        ...RESTORE_BUILD(BUILD_JOB_ID, platform.name),
        ...(platform.name === "macos"
          ? [{
            name: "File info",
            run:
              `file target/${target.target}/* target/${target.target}/deps/*`,
          }]
          : []),
        {
          name: "Run tests",
          run:
            `deno run --allow-read --allow-run ./tools/ci/test.js ${target.target} \${{ matrix.shard }} ${target.test}`,
        },
      ],
    };

    tests.push(TEST_JOB_ID);

    if (target.wpt !== undefined) {
      const WPT_JOB_ID = `wpt-${target.target}-${platform.name}`;
      const WPT_JOB_NAME =
        `wpt/${platform.name} (${target.target}, shard \${{ matrix.shard }})`;
      CI.jobs[WPT_JOB_ID] = {
        name: WPT_JOB_NAME,
        "runs-on": RUNNERS[platform.name],
        "timeout-minutes": 90,
        strategy: {
          matrix: {
            shard: Array.from({ length: target.wpt }, (_, i) => i + 1),
          },
          "fail-fast": false,
        },
        needs: [BUILD_JOB_ID],
        steps: [
          ...CHECKOUT_STEPS(["./test_util/std", "./test_util/wpt"]),
          INSTALL_DENO,
          INSTALL_PYTHON,
          ...RESTORE_BUILD(BUILD_JOB_ID, platform.name),
          {
            name: "Set up WPT runner",
            run: "deno run -A --unstable ./tools/wpt.ts setup",
          },
          {
            name: "Set up hosts file",
            run: "./wpt make-hosts-file | sudo tee -a /etc/hosts",
            "working-directory": "./test_util/wpt/",
          },
          {
            name: "Run Web Platform Tests",
            run:
              `deno run -A --unstable ./tools/wpt.ts run --quiet --binary=./target/${target.target}/deno --shard=\${{ matrix.shard }}/${target.wpt}`,
          },
        ],
      };

      tests.push(WPT_JOB_ID);
    }

    // TODO(lucacasonato): wpt upload
  }

  // TODO(lucacasonato): run benchmarks

  // TODO(lucacasonato): upload benchmarks
}

// TODO(lucacasonato): upload release (notes and binaries)

const uploadCanary = CI.jobs["upload-canary"] = {
  name: "upload canary",
  "runs-on": RUNNERS.linux,
  "timeout-minutes": 10,
  if: MAIN_CONDITION,
  needs: releases.map(([jobId]) => jobId),
  steps: [
    {
      name: "Setup gcloud",
      uses: "google-github-actions/setup-gcloud@v0",
      with: {
        project_id: "denoland",
        service_account_key: "${{ secrets.GCP_SA_KEY }}",
        export_default_credentials: true,
      },
    },
    {
      name: "Create canary version manifest",
      run: "echo ${{ github.sha }} > canary-latest.txt",
    },
  ],
};

const TARGET_TUPLE = {
  "linux": "deno-x86_64-unknown-linux-gnu",
  "macos": "deno-x86_64-apple-darwin",
  "windows": "deno-x86_64-pc-windows-msvc",
};

const BUCKET_NAME = "dl.deno.land";

for (const [jobId, platform] of releases) {
  const ZIP = `${TARGET_TUPLE[platform]}.zip`;
  uploadCanary.steps.push({
    name: `Download artifacts (${platform})`,
    uses: "actions/download-artifact@v3",
    with: {
      name: jobId,
      path: `${platform}/`,
    },
  });
  uploadCanary.steps.push({
    name: `Unpack artifacts (${platform})`,
    run: ARTIFACT_PATHS.map((path) =>
      `${
        platform === "macos" ? "gtar" : "tar"
      } -xpf ${platform}/${path} -C ${platform}`
    ).join("\n"),
  });
  uploadCanary.steps.push({
    name: `Create zip (${platform})`,
    run: `cd ${platform}/target/release && zip -r ../../../${ZIP} deno${
      platform === "windows" ? ".exe" : ""
    }`,
  });
  uploadCanary.steps.push({
    name: `Upload zip to ${BUCKET_NAME} (${platform})`,
    run:
      `gsutil -h "Cache-Control: public, max-age=3600" cp ${ZIP} gs://${BUCKET_NAME}/canary/\${{ github.sha }}/`,
  });
}

uploadCanary.steps.push({
  name: "Upload canary version manifest",
  run:
    `gsutil -h "Cache-Control: no-cache" cp canary-latest.txt gs://${BUCKET_NAME}/canary-latest.txt`,
});

const CI_YAML = YAML.stringify(CI, {
  noRefs: true,
  lineWidth: 10000,
  noCompatMode: true,
});
const HEADER =
  `# THIS FILE IS AUTO-GENERATED. DO NOT EDIT.\n# This CI configuration is generated by ./tools/ci/generate.ts.\n\n`;
const FORMATTED = prettier.format(HEADER + CI_YAML, {
  parser: "yaml",
  plugins: [prettierYaml],
});
Deno.writeTextFileSync(".github/workflows/ci.yml", FORMATTED);
