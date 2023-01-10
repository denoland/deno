import * as yaml from "https://deno.land/std@0.171.0/encoding/yaml.ts";

const ci = {
  name: "ci",
  on: ["push", "pull_request"],
  concurrency: {
    group:
      "${{ github.workflow }}-${{ !contains(github.event.pull_request.labels.*.name, 'test-flaky-ci') && github.head_ref || github.run_id }}",
    "cancel-in-progress": true,
  },
  jobs: {
    build: {
      name: "${{ matrix.job }} ${{ matrix.profile }} ${{ matrix.os }}",
      if:
        "github.event_name == 'push' ||\n!startsWith(github.event.pull_request.head.label, 'denoland:')\n",
      "runs-on": "${{ matrix.os }}",
      "timeout-minutes": 120,
      strategy: {
        matrix: {
          include: [
            {
              os: "macos-12",
              job: "test",
              profile: "fastci",
            },
            {
              os: "macos-12",
              job: "test",
              profile: "release",
            },
            {
              os:
                "${{ github.repository == 'denoland/deno' && 'windows-2019-xl' || 'windows-2019' }}",
              job: "test",
              profile: "fastci",
            },
            {
              os:
                "${{ github.repository == 'denoland/deno' && 'windows-2019-xl' || 'windows-2019' }}",
              job: "test",
              profile: "release",
            },
            {
              os:
                "${{ github.repository == 'denoland/deno' && 'ubuntu-20.04-xl' || 'ubuntu-20.04' }}",
              job: "test",
              profile: "release",
              use_sysroot: true,
            },
            {
              os:
                "${{ github.repository == 'denoland/deno' && 'ubuntu-20.04-xl' || 'ubuntu-20.04' }}",
              job: "bench",
              profile: "release",
              use_sysroot: true,
            },
            {
              os:
                "${{ github.repository == 'denoland/deno' && 'ubuntu-20.04-xl' || 'ubuntu-20.04' }}",
              job: "test",
              profile: "debug",
              use_sysroot: true,
            },
            {
              os:
                "${{ github.repository == 'denoland/deno' && 'ubuntu-20.04-xl' || 'ubuntu-20.04' }}",
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
      steps: [
        {
          name: "Configure git",
          run:
            "git config --global core.symlinks true\ngit config --global fetch.parallel 32\n",
        },
        {
          name: "Clone repository",
          uses: "actions/checkout@v3",
          with: {
            // Use depth > 1, because sometimes we need to rebuild main and if
            // other commits have landed it will become impossible to rebuild if
            // the checkout is too shallow.
            "fetch-depth": 5,
            submodules: "recursive",
          },
        },
        {
          name: "Create source tarballs (release, linux)",
          if:
            "startsWith(matrix.os, 'ubuntu') &&\nmatrix.profile == 'release' &&\nmatrix.job == 'test' &&\ngithub.repository == 'denoland/deno' &&\nstartsWith(github.ref, 'refs/tags/')\n",
          run:
            'mkdir -p target/release\ntar --exclude=".git*" --exclude=target --exclude=third_party/prebuilt \\\n    -czvf target/release/deno_src.tar.gz -C .. deno\n',
        },
        { uses: "dtolnay/rust-toolchain@stable" },
        {
          name: "Install Deno",
          if: "matrix.job == 'lint' || matrix.job == 'test'",
          uses: "denoland/setup-deno@v1",
          with: { "deno-version": "v1.x" },
        },
        {
          name: "Install Python",
          uses: "actions/setup-python@v4",
          with: { "python-version": 3.8 },
        },
        {
          name: "Install Node",
          uses: "actions/setup-node@v3",
          with: { "node-version": 17 },
        },
        {
          name: "Remove unused versions of Python",
          if: "startsWith(matrix.os, 'windows')",
          run:
            '$env:PATH -split ";" |\n  Where-Object { Test-Path "$_\\python.exe" } |\n  Select-Object -Skip 1 |\n  ForEach-Object { Move-Item "$_" "$_.disabled" }',
        },
        {
          name: "Setup gcloud (unix)",
          if:
            "runner.os != 'Windows' &&\nmatrix.profile == 'release' &&\nmatrix.job == 'test' &&\ngithub.repository == 'denoland/deno' &&\n(github.ref == 'refs/heads/main' ||\nstartsWith(github.ref, 'refs/tags/'))\n",
          uses: "google-github-actions/setup-gcloud@v0",
          with: {
            project_id: "denoland",
            service_account_key: "${{ secrets.GCP_SA_KEY }}",
            export_default_credentials: true,
          },
        },
        {
          name: "Setup gcloud (windows)",
          if:
            "runner.os == 'Windows' &&\nmatrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\n(github.ref == 'refs/heads/main' ||\nstartsWith(github.ref, 'refs/tags/'))\n",
          uses: "google-github-actions/setup-gcloud@v0",
          env: { CLOUDSDK_PYTHON: "${{env.pythonLocation}}\\python.exe" },
          with: {
            project_id: "denoland",
            service_account_key: "${{ secrets.GCP_SA_KEY }}",
            export_default_credentials: true,
          },
        },
        {
          name: "Configure canary build",
          if:
            "matrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\ngithub.ref == 'refs/heads/main'\n",
          shell: "bash",
          run: 'echo "DENO_CANARY=true" >> $GITHUB_ENV',
        },
        {
          name: "Set up incremental LTO and sysroot build",
          if: "matrix.use_sysroot",
          run:
            '# Avoid running man-db triggers, which sometimes takes several minutes\n# to complete.\nsudo apt-get remove --purge -y man-db\n\n# Install clang-15, lld-15, and debootstrap.\necho "deb http://apt.llvm.org/focal/ llvm-toolchain-focal-15 main" |\n  sudo dd of=/etc/apt/sources.list.d/llvm-toolchain-focal-15.list\ncurl https://apt.llvm.org/llvm-snapshot.gpg.key |\n  gpg --dearmor                                 |\nsudo dd of=/etc/apt/trusted.gpg.d/llvm-snapshot.gpg\nsudo apt-get update\nsudo apt-get install --no-install-recommends debootstrap     \\\n                                             clang-15 lld-15\n\n# Create ubuntu-16.04 sysroot environment, which is used to avoid\n# depending on a very recent version of glibc.\n# `libc6-dev` is required for building any C source files.\n# `file` and `make` are needed to build libffi-sys.\n# `curl` is needed to build rusty_v8.\nsudo debootstrap                                     \\\n  --include=ca-certificates,curl,file,libc6-dev,make \\\n  --no-merged-usr --variant=minbase xenial /sysroot  \\\n  http://azure.archive.ubuntu.com/ubuntu\nsudo mount --rbind /dev /sysroot/dev\nsudo mount --rbind /sys /sysroot/sys\nsudo mount --rbind /home /sysroot/home\nsudo mount -t proc /proc /sysroot/proc\n\n# Configure the build environment. Both Rust and Clang will produce\n# llvm bitcode only, so we can use lld\'s incremental LTO support.\ncat >> $GITHUB_ENV << __0\nCARGO_PROFILE_BENCH_INCREMENTAL=false\nCARGO_PROFILE_BENCH_LTO=false\nCARGO_PROFILE_RELEASE_INCREMENTAL=false\nCARGO_PROFILE_RELEASE_LTO=false\nRUSTFLAGS<<__1\n  -C linker-plugin-lto=true\n  -C linker=clang-15\n  -C link-arg=-fuse-ld=lld-15\n  -C link-arg=--sysroot=/sysroot\n  -C link-arg=-Wl,--allow-shlib-undefined\n  -C link-arg=-Wl,--thinlto-cache-dir=$(pwd)/target/release/lto-cache\n  -C link-arg=-Wl,--thinlto-cache-policy,cache_size_bytes=700m\n  ${{ env.RUSTFLAGS }}\n__1\nRUSTDOCFLAGS<<__1\n  -C linker-plugin-lto=true\n  -C linker=clang-15\n  -C link-arg=-fuse-ld=lld-15\n  -C link-arg=--sysroot=/sysroot\n  -C link-arg=-Wl,--allow-shlib-undefined\n  -C link-arg=-Wl,--thinlto-cache-dir=$(pwd)/target/release/lto-cache\n  -C link-arg=-Wl,--thinlto-cache-policy,cache_size_bytes=700m\n  ${{ env.RUSTFLAGS }}\n__1\nCC=clang-15\nCFLAGS=-flto=thin --sysroot=/sysroot\n__0\n',
        },
        {
          name: "Log versions",
          shell: "bash",
          run:
            'node -v\npython --version\nrustc --version\ncargo --version\n# Deno is installed when linting.\nif [ "${{ matrix.job }}" == "lint" ]\nthen\n  deno --version\nfi\n',
        },
        {
          name: "Cache Cargo home",
          uses: "actions/cache@v3",
          with: {
            // See https://doc.rust-lang.org/cargo/guide/cargo-home.html#caching-the-cargo-home-in-ci
            path:
              "~/.cargo/registry/index\n~/.cargo/registry/cache\n~/.cargo/git/db\n",
            key:
              "18-cargo-home-${{ matrix.os }}-${{ hashFiles('Cargo.lock') }}",
          },
        },
        {
          // In main branch, always creates fresh cache
          name: "Cache build output (main)",
          uses: "actions/cache/save@v3",
          if:
            "(matrix.profile == 'release' || matrix.profile == 'fastci') && github.ref == 'refs/heads/main'",
          with: {
            path:
              "./target\n!./target/*/gn_out\n!./target/*/*.zip\n!./target/*/*.tar.gz\n",
            key:
              "18-cargo-target-${{ matrix.os }}-${{ matrix.profile }}-${{ github.sha }}\n",
          },
        },
        {
          // Restore cache from the latest 'main' branch build.
          name: "Cache build output (PR)",
          uses: "actions/cache/restore@v3",
          if:
            "github.ref != 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')",
          with: {
            path:
              "./target\n!./target/*/gn_out\n!./target/*/*.zip\n!./target/*/*.tar.gz\n",
            key: "never_saved",
            "restore-keys":
              "18-cargo-target-${{ matrix.os }}-${{ matrix.profile }}-\n",
          },
        },
        {
          name: "Apply and update mtime cache",
          if: "matrix.profile == 'release'",
          uses: "./.github/mtime_cache",
          with: { "cache-path": "./target" },
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
          run:
            "if [ ! -d ~/.cargo/registry/index/github.com-1ecc6299db9ec823/.git ]\nthen\n  git clone --depth 1 --no-checkout                      \\\n            https://github.com/rust-lang/crates.io-index \\\n            ~/.cargo/registry/index/github.com-1ecc6299db9ec823\nfi\n",
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
          if:
            "(matrix.job == 'test' || matrix.job == 'bench') &&\nmatrix.profile == 'debug'\n",
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
          if:
            "(matrix.job == 'test' || matrix.job == 'bench') &&\nmatrix.profile == 'release' && (matrix.use_sysroot ||\n(github.repository == 'denoland/deno' &&\n(github.ref == 'refs/heads/main' ||\nstartsWith(github.ref, 'refs/tags/'))))\n",
          run: "cargo build --release --locked --all-targets",
        },
        {
          name: "Upload PR artifact (linux)",
          if:
            "matrix.job == 'test' &&\nmatrix.profile == 'release' && (matrix.use_sysroot ||\n(github.repository == 'denoland/deno' &&\n(github.ref == 'refs/heads/main' ||\nstartsWith(github.ref, 'refs/tags/'))))\n",
          uses: "actions/upload-artifact@v3",
          with: {
            name: "deno-${{ github.event.number }}",
            path: "target/release/deno",
          },
        },
        {
          name: "Pre-release (linux)",
          if:
            "startsWith(matrix.os, 'ubuntu') &&\nmatrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno'\n",
          run:
            "cd target/release\nzip -r deno-x86_64-unknown-linux-gnu.zip deno\n./deno types > lib.deno.d.ts\n",
        },
        {
          name: "Pre-release (mac)",
          if:
            "startsWith(matrix.os, 'macOS') &&\nmatrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\n(github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))\n",
          run: "cd target/release\nzip -r deno-x86_64-apple-darwin.zip deno\n",
        },
        {
          name: "Pre-release (windows)",
          if:
            "startsWith(matrix.os, 'windows') &&\nmatrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\n(github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))\n",
          run:
            "Compress-Archive -CompressionLevel Optimal -Force -Path target/release/deno.exe -DestinationPath target/release/deno-x86_64-pc-windows-msvc.zip\n",
        },
        {
          name: "Upload canary to dl.deno.land (unix)",
          if:
            "runner.os != 'Windows' &&\nmatrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\ngithub.ref == 'refs/heads/main'\n",
          run:
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/canary/$(git rev-parse HEAD)/\n',
        },
        {
          name: "Upload canary to dl.deno.land (windows)",
          if:
            "runner.os == 'Windows' &&\nmatrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\ngithub.ref == 'refs/heads/main'\n",
          env: { CLOUDSDK_PYTHON: "${{env.pythonLocation}}\\python.exe" },
          shell: "bash",
          run:
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/canary/$(git rev-parse HEAD)/\n',
        },
        {
          name: "Test debug",
          if:
            "matrix.job == 'test' && matrix.profile == 'debug' &&\n!startsWith(github.ref, 'refs/tags/')\n",
          run: "cargo test --locked --doc\ncargo test --locked\n",
        },
        {
          name: "Test fastci",
          if: "(matrix.job == 'test' && matrix.profile == 'fastci')",
          run: "cargo test --locked",
          env: { CARGO_PROFILE_DEV_DEBUG: 0 },
        },
        {
          name: "Test release",
          if:
            "matrix.job == 'test' && matrix.profile == 'release' &&\n(matrix.use_sysroot || (\ngithub.repository == 'denoland/deno' &&\ngithub.ref == 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')))\n",
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
          env: { NO_COLOR: 1 },
        },
        {
          // Verify that the binary actually works in the Ubuntu-16.04 sysroot.
          name: "Check deno binary (in sysroot)",
          if: "matrix.profile == 'release' && matrix.use_sysroot",
          run: 'sudo chroot /sysroot "$(pwd)/target/release/deno" --version',
        },
        {
          // TODO(ry): Because CI is so slow on for OSX and Windows, we currently
          //           run the Web Platform tests only on Linux.
          name: "Configure hosts file for WPT",
          if: "startsWith(matrix.os, 'ubuntu') && matrix.job == 'test'",
          run: "./wpt make-hosts-file | sudo tee -a /etc/hosts",
          "working-directory": "test_util/wpt/",
        },
        {
          name: "Run web platform tests (debug)",
          if:
            "startsWith(matrix.os, 'ubuntu') && matrix.job == 'test' &&\nmatrix.profile == 'debug' &&\ngithub.ref == 'refs/heads/main'\n",
          env: { DENO_BIN: "./target/debug/deno" },
          run:
            'deno run --allow-env --allow-net --allow-read --allow-run \\\n        --allow-write --unstable                         \\\n        --lock=tools/deno.lock.json                      \\\n        ./tools/wpt.ts setup\ndeno run --allow-env --allow-net --allow-read --allow-run \\\n         --allow-write --unstable                         \\\n         --lock=tools/deno.lock.json              \\\n         ./tools/wpt.ts run --quiet --binary="$DENO_BIN"\n',
        },
        {
          name: "Run web platform tests (release)",
          if:
            "startsWith(matrix.os, 'ubuntu') && matrix.job == 'test' &&\nmatrix.profile == 'release' && !startsWith(github.ref, 'refs/tags/')\n",
          env: { DENO_BIN: "./target/release/deno" },
          run:
            'deno run --allow-env --allow-net --allow-read --allow-run \\\n         --allow-write --unstable                         \\\n         --lock=tools/deno.lock.json                      \\\n         ./tools/wpt.ts setup\ndeno run --allow-env --allow-net --allow-read --allow-run \\\n         --allow-write --unstable                         \\\n         --lock=tools/deno.lock.json                      \\\n         ./tools/wpt.ts run --quiet --release             \\\n                            --binary="$DENO_BIN"          \\\n                            --json=wpt.json               \\\n                            --wptreport=wptreport.json\n',
        },
        {
          name: "Upload wpt results to dl.deno.land",
          "continue-on-error": true,
          if:
            "runner.os == 'Linux' &&\nmatrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\ngithub.ref == 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')\n",
          run:
            'gzip ./wptreport.json\ngsutil -h "Cache-Control: public, max-age=3600" cp ./wpt.json gs://dl.deno.land/wpt/$(git rev-parse HEAD).json\ngsutil -h "Cache-Control: public, max-age=3600" cp ./wptreport.json.gz gs://dl.deno.land/wpt/$(git rev-parse HEAD)-wptreport.json.gz\necho $(git rev-parse HEAD) > wpt-latest.txt\ngsutil -h "Cache-Control: no-cache" cp wpt-latest.txt gs://dl.deno.land/wpt-latest.txt\n',
        },
        {
          name: "Upload wpt results to wpt.fyi",
          "continue-on-error": true,
          if:
            "runner.os == 'Linux' &&\nmatrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\ngithub.ref == 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')\n",
          env: {
            WPT_FYI_USER: "deno",
            WPT_FYI_PW: "${{ secrets.WPT_FYI_PW }}",
            GITHUB_TOKEN: "${{ secrets.DENOBOT_PAT }}",
          },
          run:
            "./target/release/deno run --allow-all --lock=tools/deno.lock.json \\\n    ./tools/upload_wptfyi.js $(git rev-parse HEAD) --ghstatus\n",
        },
        {
          name: "Run benchmarks",
          if: "matrix.job == 'bench' && !startsWith(github.ref, 'refs/tags/')",
          run: "cargo bench --locked",
        },
        {
          name: "Post Benchmarks",
          if:
            "matrix.job == 'bench' &&\ngithub.repository == 'denoland/deno' &&\ngithub.ref == 'refs/heads/main' && !startsWith(github.ref, 'refs/tags/')\n",
          env: { DENOBOT_PAT: "${{ secrets.DENOBOT_PAT }}" },
          run:
            'git clone --depth 1 --branch gh-pages                             \\\n    https://${DENOBOT_PAT}@github.com/denoland/benchmark_data.git \\\n    gh-pages\n./target/release/deno run --allow-all --unstable \\\n    ./tools/build_benchmark_jsons.js --release\ncd gh-pages\ngit config user.email "propelml@gmail.com"\ngit config user.name "denobot"\ngit add .\ngit commit --message "Update benchmarks"\ngit push origin gh-pages\n',
        },
        {
          name: "Build product size info",
          if:
            "matrix.job != 'lint' && matrix.profile != 'fastci' && github.repository == 'denoland/deno' && (github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))",
          run:
            'du -hd1 "./target/${{ matrix.profile }}"\ndu -ha  "./target/${{ matrix.profile }}/deno"\n',
        },
        {
          name: "Worker info",
          if: "matrix.job == 'bench'",
          run: "cat /proc/cpuinfo\ncat /proc/meminfo\n",
        },
        {
          name: "Upload release to dl.deno.land (unix)",
          if:
            "runner.os != 'Windows' &&\nmatrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\nstartsWith(github.ref, 'refs/tags/')\n",
          run:
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/\n',
        },
        {
          name: "Upload release to dl.deno.land (windows)",
          if:
            "runner.os == 'Windows' &&\nmatrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\nstartsWith(github.ref, 'refs/tags/')\n",
          env: { CLOUDSDK_PYTHON: "${{env.pythonLocation}}\\python.exe" },
          shell: "bash",
          run:
            'gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/release/${GITHUB_REF#refs/*/}/\n',
        },
        {
          name: "Create release notes",
          shell: "bash",
          if:
            "matrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\nstartsWith(github.ref, 'refs/tags/')\n",
          run:
            "export PATH=$PATH:$(pwd)/target/release\n./tools/release/05_create_release_notes.ts\n",
        },
        {
          name: "Upload release to GitHub",
          uses: "softprops/action-gh-release@v0.1.15",
          if:
            "matrix.job == 'test' &&\nmatrix.profile == 'release' &&\ngithub.repository == 'denoland/deno' &&\nstartsWith(github.ref, 'refs/tags/')\n",
          env: { GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}" },
          with: {
            files:
              "target/release/deno-x86_64-pc-windows-msvc.zip\ntarget/release/deno-x86_64-unknown-linux-gnu.zip\ntarget/release/deno-x86_64-apple-darwin.zip\ntarget/release/deno_src.tar.gz\ntarget/release/lib.deno.d.ts\n",
            body_path: "target/release/release-notes.md",
            draft: true,
          },
        },
      ],
    },
    "publish-canary": {
      name: "publish canary",
      "runs-on": "ubuntu-20.04",
      needs: ["build"],
      if:
        "github.repository == 'denoland/deno' && github.ref == 'refs/heads/main'",
      steps: [{
        name: "Setup gcloud",
        uses: "google-github-actions/setup-gcloud@v0",
        with: {
          project_id: "denoland",
          service_account_key: "${{ secrets.GCP_SA_KEY }}",
          export_default_credentials: true,
        },
      }, {
        name: "Upload canary version file to dl.deno.land",
        run:
          'echo ${{ github.sha }} > canary-latest.txt\ngsutil -h "Cache-Control: no-cache" cp canary-latest.txt gs://dl.deno.land/canary-latest.txt\n',
      }],
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
