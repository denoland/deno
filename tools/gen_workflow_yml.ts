#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-run

const RUST_VERSION = "1.50.0";
const DENO_VERSION = "v1.7.2";

const slowRunners = {
  "linux": "ubuntu-18.04",
  "macos": "macos-10.15",
  "windows": "windows-2019",
};

const fastRunners = {
  "linux":
    "${{ github.repository == 'denoland/deno' && 'ubuntu-latest-xl' || 'ubuntu-18.04' }}",
  "macos": "macos-10.15",
  "windows": "windows-2019",
};

const targets = {
  "linux": "x86_64-unknown-linux-gnu",
  "macos": "x86_64-apple-darwin",
  "windows": "denort-x86_64-pc-windows-msvc",
};

const env = {
  "CARGO_INCREMENTAL": "0",
  "RUST_BACKTRACE": "full",
  "CARGO_TERM_COLOR": "always",
};

const platforms = ["linux", "macos", "windows"] as const;
const kinds = ["release", "debug"] as const;

const chechout = [
  {
    name: "Configure git",
    run: "git config --global core.symlinks true",
  },
  {
    name: "Checkout repository",
    uses: "actions/checkout@v2",
    with: {
      "fetch-depth": 5,
      submodules: true,
    },
  },
];

function buildCache(os: string, kind: string) {
  return {
    name: "Build cache",
    uses: "actions/cache@v2",
    with: {
      path: [
        "~/.cargo/registry",
        "~/.cargo/git",
        ".cargo_home",
        "target/*/.*",
        "target/*/build",
        "target/*/deps",
      ].join("\n"),
      key: `${os}-${kind}-\${{ hashFiles('Cargo.lock') }}`,
      "restore-keys": `${os}-${kind}-`,
    },
  };
}

const setupRust = {
  name: "Setup Rust",
  uses: "actions-rs/toolchain@v1",
  with: {
    default: true,
    override: true,
    toolchain: RUST_VERSION,
  },
};

const setupDeno = {
  name: "Setup Deno",
  run: [
    `curl -fsSL https://deno.land/x/install/install.sh | sh -s ${DENO_VERSION}`,
    `echo "$HOME/.deno/bin" >> $\${{ runner.os == 'Windows' && 'env:' || '' }}GITHUB_PATH`,
  ].join("\n"),
};

function generateBuildJobs(): Record<string, unknown> {
  const jobs: Record<string, unknown> = {};

  for (const os of platforms) {
    for (const kind of kinds) {
      if (os != "linux" && kind == "debug") continue;

      const zipSourceStep = {
        name: "Package sourcecode",
        run: [
          "mkdir -p target/release",
          `tar --exclude=.cargo_home --exclude=".git*" --exclude=target --exclude=third_party/prebuilt -czf target/release/deno_src.tar.gz -C .. deno`,
        ].join("\n"),
      };
      const packageStep = {
        name: "Package",
        "working-directory": "target/release",
        run: (os == "windows"
          ? [
            "Compress-Archive -CompressionLevel Optimal -Force -Path deno.exe -DestinationPath deno-x86_64-pc-windows-msvc.zip",
            "Compress-Archive -CompressionLevel Optimal -Force -Path denort.exe -DestinationPath denort-x86_64-pc-windows-msvc.zip",
          ]
          : [
            `zip -r deno-${targets[os]}.zip deno`,
            `zip -r denort-${targets[os]}.zip denort`,
            ...(os == "linux" ? ["./deno types > lib.deno.d.ts"] : []),
          ]).join("\n"),
      };
      const uploadPackageStep = {
        name: "Upload package artifacts",
        uses: "actions/upload-artifact@v2",
        with: {
          name: "package",
          path: [
            "target/release/deno-x86_64-unknown-linux-gnu.zip",
            "target/release/deno-x86_64-pc-windows-msvc.zip",
            "target/release/deno-x86_64-apple-darwin.zip",
            "target/release/denort-x86_64-unknown-linux-gnu.zip",
            "target/release/denort-x86_64-pc-windows-msvc.zip",
            "target/release/denort-x86_64-apple-darwin.zip",
            "target/release/deno_src.tar.gz",
            "target/release/lib.deno.d.ts",
          ].join("\n"),
          "retention-days": 7,
        },
      };

      jobs[`build_${os}_${kind}`] = {
        name: `build / ${os} / ${kind}`,
        "runs-on": fastRunners[os],
        "timeout-minutes": 60,
        env,
        steps: [
          ...chechout,
          ...(kind == "release" && os == "linux" ? [zipSourceStep] : []),
          {
            name: "Configure canary",
            if: "!startsWith(github.ref, 'refs/tags/')",
            shell: "bash",
            run: "echo 'DENO_CANARY=true' >> $GITHUB_ENV",
          },
          buildCache(os, kind),
          setupRust,
          {
            name: "Log versions",
            run: [
              "rustc --version",
              "cargo --version",
            ].join("\n"),
          },
          {
            name: `Build (${kind})`,
            uses: "actions-rs/cargo@v1",
            with: {
              "use-cross": false,
              command: "build",
              args: `--locked --all-targets ${
                kind == "release" ? "--release" : ""
              }`,
            },
          },
          {
            name: "Upload build artifact",
            uses: "actions/upload-artifact@v2",
            with: {
              name: `build-${os}-${kind}`,
              path: [
                `target/${kind}/deno${os == "windows" ? ".exe" : ""}`,
                `target/${kind}/denort${os == "windows" ? ".exe" : ""}`,
                `target/${kind}/test_server${os == "windows" ? ".exe" : ""}`,
              ].join("\n"),
            },
          },
          ...(kind == "release" ? [packageStep, uploadPackageStep] : []),
        ],
      };
    }
  }

  return jobs;
}

function downloadBuildCache(os: string, kind: string): unknown[] {
  return [
    {
      name: "Download build artifacts",
      uses: "actions/download-artifact@v2",
      with: {
        name: `build-${os}-${kind}`,
        path: `target/${kind}/`,
      },
    },
    ...(os != "windows"
      ? [
        {
          name: "Make build artifacts executable",
          run: [
            `chmod +x target/${kind}/deno${os == "windows" ? ".exe" : ""}`,
            `chmod +x target/${kind}/denort${os == "windows" ? ".exe" : ""}`,
            `chmod +x target/${kind}/test_server${
              os == "windows" ? ".exe" : ""
            }`,
          ].join("\n"),
        },
      ]
      : []),
  ];
}

function generateTestJobs(): Record<string, unknown> {
  const jobs: Record<string, unknown> = {};

  for (const os of platforms) {
    for (const kind of kinds) {
      if (os != "linux" && kind == "debug") continue;

      jobs[`test_${os}_${kind}`] = {
        name: `test / ${os} / ${kind}`,
        "runs-on": slowRunners[os],
        "timeout-minutes": 60,
        needs: [`build_${os}_${kind}`],
        env,
        steps: [
          ...chechout,
          setupRust,
          setupDeno,
          {
            name: "Log versions",
            run: [
              "rustc --version",
              "cargo --version",
              "deno --version",
            ].join("\n"),
          },
          buildCache(os, kind),
          ...downloadBuildCache(os, kind),
          ...(kind == "release"
            ? [
              {
                name: "Test (release)",
                run: "cargo test --release --locked --all-targets",
              },
            ]
            : [
              {
                name: "Test (debug)",
                run: [
                  "cargo test --locked --doc",
                  "cargo test --locked --all-targets",
                ].join("\n"),
              },
            ]),
        ],
      };
    }
  }

  return jobs;
}

const ci = {
  name: "ci",
  // FIXME
  on: [/*"push",*/ "pull_request"],
  jobs: {
    ...generateBuildJobs(),
    ...generateTestJobs(),
  },
};

const str = JSON.stringify(ci, undefined, 2);
await Deno.writeTextFile(
  "./.github/workflows/ci.yml",
  `# THIS FILE IS AUTOGENERATED USING ./tools/get_workflow.yml\n${str}`,
);
