#!/usr/bin/env -S deno run --check --allow-write=. --allow-read=. --lock=./tools/deno.lock.json
// Copyright 2018-2026 the Deno authors. MIT license.
import {
  createWorkflow,
  defineMatrix,
  job,
  step,
} from "jsr:@david/gagen@0.3.1";

// === build job ===

const buildConfigureGit = step({
  name: "Configure git",
  run: [
    "git config --global core.symlinks true",
    "git config --global fetch.parallel 32",
  ],
});

const buildClone = step.dependsOn(buildConfigureGit)({
  name: "Clone repository",
  uses: "actions/checkout@v6",
  with: { submodules: "recursive" },
});

const buildInstallDeno = step.dependsOn(buildClone)({
  name: "Install Deno",
  uses: "denoland/setup-deno@v2",
  with: { "deno-version": "v2.7.1" },
});

const buildNpm = step.dependsOn(buildInstallDeno)({
  name: "Build npm packages",
  run: "./tools/release/npm/build.ts ${{ inputs.version || '' }}",
});

const tarDist = step.dependsOn(buildNpm)({
  name: "Tar npm dist (preserves permissions)",
  run:
    "tar cf tools/release/npm/dist.tar -C tools/release/npm --exclude='node_modules' dist",
});

const uploadDist = step.dependsOn(tarDist)({
  name: "Upload npm dist artifact",
  uses: "actions/upload-artifact@v6",
  with: {
    name: "npm-dist",
    path: "tools/release/npm/dist.tar",
    "retention-days": 1,
  },
});

const buildJob = job("build", {
  name: "npm build",
  runsOn: "ubuntu-latest",
  timeoutMinutes: 30,
  steps: [uploadDist],
});

// === test job ===

const testMatrix = defineMatrix({
  runner: ["ubuntu-latest", "macos-latest", "windows-latest"],
});

const testClone = step({
  name: "Clone repository",
  uses: "actions/checkout@v6",
  with: { submodules: false },
});

const testInstallNode = step.dependsOn(testClone)({
  name: "Install Node",
  uses: "actions/setup-node@v6",
  with: { "node-version": "24.x" },
});

const downloadArtifact = step.dependsOn(testInstallNode)({
  name: "Download npm dist artifact",
  uses: "actions/download-artifact@v6",
  with: {
    name: "npm-dist",
    path: "tools/release/npm",
  },
});

const extractDist = step.dependsOn(downloadArtifact)({
  name: "Extract npm dist",
  run: "tar xf tools/release/npm/dist.tar -C tools/release/npm",
});

const writeVerdaccioConfig = step.dependsOn(extractDist)({
  name: "Write Verdaccio config",
  run: [
    'mkdir -p "${{ runner.temp }}/verdaccio/storage"',
    "cat > \"${{ runner.temp }}/verdaccio/config.yaml\" << 'HEREDOC'",
    "storage: ./storage",
    "uplinks: {}",
    "packages:",
    "  '@deno/*':",
    "    access: $all",
    "    publish: $all",
    "  'deno':",
    "    access: $all",
    "    publish: $all",
    "  '**':",
    "    access: $all",
    "    publish: $all",
    "max_body_size: 200mb",
    "log: { type: stdout, format: pretty, level: warn }",
    "HEREDOC",
  ],
});

const startVerdaccio = step.dependsOn(writeVerdaccioConfig)({
  name: "Start Verdaccio",
  run: [
    "npx verdaccio@6 \\",
    '  --config "${{ runner.temp }}/verdaccio/config.yaml" --listen 4873 &',
    "for i in $(seq 1 30); do",
    "  if curl -s http://localhost:4873/-/ping > /dev/null 2>&1; then",
    '    echo "Verdaccio is ready"',
    "    break",
    "  fi",
    "  sleep 1",
    "done",
  ],
});

const installPnpm = step.dependsOn(startVerdaccio)({
  name: "Install pnpm",
  run: [
    "npm install -g pnpm",
    'PNPM_HOME="${{ runner.temp }}/pnpm-global"',
    'mkdir -p "$PNPM_HOME"',
    'echo "PNPM_HOME=$PNPM_HOME" >> "$GITHUB_ENV"',
    'echo "$PNPM_HOME" >> "$GITHUB_PATH"',
  ],
});

const configureNpm = step.dependsOn(installPnpm)({
  name: "Configure npm to use Verdaccio",
  run: [
    "npm config set registry http://localhost:4873/",
    "npm config set //localhost:4873/:_authToken dummy-token",
  ],
});

const publishVerdaccio = step.dependsOn(configureNpm)({
  name: "Publish packages to Verdaccio",
  id: "publish-verdaccio",
  outputs: ["version"],
  run: [
    'DIST_DIR="tools/release/npm/dist"',
    'for pkg_dir in "$DIST_DIR"/@deno/*/; do',
    '  echo "Publishing $(basename "$pkg_dir") to Verdaccio..."',
    '  (cd "$pkg_dir" && npm publish --registry http://localhost:4873/)',
    "done",
    'echo "Publishing deno to Verdaccio..."',
    '(cd "$DIST_DIR/deno" && npm publish --registry http://localhost:4873/)',
    "VERSION=$(node -p \"require('./$DIST_DIR/deno/package.json').version\")",
    'echo "version=$VERSION" >> "$GITHUB_OUTPUT"',
  ],
});

const testNpmInstall = step.dependsOn(publishVerdaccio)({
  name: "Test npm install deno",
  run: [
    'TEST_DIR="${{ runner.temp }}/npm-test"',
    'mkdir -p "$TEST_DIR"',
    'cd "$TEST_DIR"',
    "npm init -y",
    'EXPECTED_VERSION="deno ${{ steps.publish-verdaccio.outputs.version }}"',
    "npm install deno@${{ steps.publish-verdaccio.outputs.version }}",
    'ACTUAL="$(npx deno -v)"',
    'echo "$ACTUAL"',
    '[ "$ACTUAL" = "$EXPECTED_VERSION" ] || { echo "Version mismatch: expected \'$EXPECTED_VERSION\', got \'$ACTUAL\'"; exit 1; }',
  ],
});

const testBinCjs = step.dependsOn(testNpmInstall)({
  name: "Test deno via bin.cjs fallback",
  run: [
    'cd "${{ runner.temp }}/npm-test"',
    "node node_modules/deno/bin.cjs eval \"console.log('npm package works')\"",
  ],
});

const testReadonlyFs = step.dependsOn(testBinCjs)({
  name: "Test deno via simulated readonly file system",
  run: [
    'TEST_DIR="${{ runner.temp }}/readonly-test"',
    'mkdir -p "$TEST_DIR"',
    'cd "$TEST_DIR"',
    "npm init -y",
    "npm install deno@${{ steps.publish-verdaccio.outputs.version }}",
    "rm -f node_modules/deno/deno*",
    "DENO_SIMULATED_READONLY_FILE_SYSTEM=1 node node_modules/deno/bin.cjs -v",
  ],
});

const testNpmGlobalInstall = step.dependsOn(testReadonlyFs)({
  name: "Test npm global install deno",
  run: [
    'EXPECTED_VERSION="deno ${{ steps.publish-verdaccio.outputs.version }}"',
    "npm install -g deno@${{ steps.publish-verdaccio.outputs.version }}",
    'ACTUAL="$(deno -v)"',
    'echo "$ACTUAL"',
    '[ "$ACTUAL" = "$EXPECTED_VERSION" ] || { echo "Version mismatch: expected \'$EXPECTED_VERSION\', got \'$ACTUAL\'"; exit 1; }',
    "deno eval \"console.log('global npm install works')\"",
    "# Verify the global bin entry points directly at the native binary, not bin.cjs",
    'if [ "$RUNNER_OS" = "Windows" ]; then',
    '  DENO_CMD="$(npm prefix -g)/deno.cmd"',
    '  grep -q "bin.cjs" "$DENO_CMD" && { echo "ERROR: deno.cmd still points to bin.cjs"; exit 1; }',
    '  echo "deno.cmd correctly points to native binary"',
    "else",
    '  DENO_LINK="$(which deno)"',
    '  LINK_TARGET="$(readlink "$DENO_LINK")"',
    '  echo "deno symlink target: $LINK_TARGET"',
    '  echo "$LINK_TARGET" | grep -q "bin.cjs" && { echo "ERROR: deno symlink still points to bin.cjs"; exit 1; }',
    '  echo "deno symlink correctly points to native binary"',
    "fi",
    "npm uninstall -g deno",
  ],
});

const testNpmGlobalIgnoreScripts = step.dependsOn(testNpmGlobalInstall)({
  name: "Test npm global install deno (--ignore-scripts)",
  run: [
    'EXPECTED_VERSION="deno ${{ steps.publish-verdaccio.outputs.version }}"',
    "npm install -g --ignore-scripts deno@${{ steps.publish-verdaccio.outputs.version }}",
    'ACTUAL="$(deno -v)"',
    'echo "$ACTUAL"',
    '[ "$ACTUAL" = "$EXPECTED_VERSION" ] || { echo "Version mismatch: expected \'$EXPECTED_VERSION\', got \'$ACTUAL\'"; exit 1; }',
    "deno eval \"console.log('global npm install works')\"",
    "npm uninstall -g deno",
  ],
});

const testPnpmLocalNoScripts = step.dependsOn(testNpmGlobalIgnoreScripts)({
  name: "Test pnpm local install deno (without postinstall)",
  run: [
    'TEST_DIR="${{ runner.temp }}/pnpm-test-no-scripts"',
    'mkdir -p "$TEST_DIR"',
    'cd "$TEST_DIR"',
    "npm init -y",
    'EXPECTED_VERSION="deno ${{ steps.publish-verdaccio.outputs.version }}"',
    "pnpm install deno@${{ steps.publish-verdaccio.outputs.version }} --registry http://localhost:4873/",
    'ACTUAL="$(node node_modules/deno/bin.cjs -v)"',
    'echo "$ACTUAL"',
    '[ "$ACTUAL" = "$EXPECTED_VERSION" ] || { echo "Version mismatch: expected \'$EXPECTED_VERSION\', got \'$ACTUAL\'"; exit 1; }',
  ],
});

const testPnpmGlobalNoScripts = step.dependsOn(testPnpmLocalNoScripts)({
  name: "Test pnpm global install deno (without postinstall)",
  run: [
    'EXPECTED_VERSION="deno ${{ steps.publish-verdaccio.outputs.version }}"',
    "pnpm install -g deno@${{ steps.publish-verdaccio.outputs.version }} --registry http://localhost:4873/",
    'ACTUAL="$(node "$PNPM_HOME/global/5/node_modules/deno/bin.cjs" -v)"',
    'echo "$ACTUAL"',
    '[ "$ACTUAL" = "$EXPECTED_VERSION" ] || { echo "Version mismatch: expected \'$EXPECTED_VERSION\', got \'$ACTUAL\'"; exit 1; }',
    "pnpm uninstall -g deno",
  ],
});

const allowPnpmBuildScripts = step.dependsOn(testPnpmGlobalNoScripts)({
  name: "Allow pnpm build scripts",
  run: "pnpm config set --global onlyBuiltDependencies '[\"*\"]'",
});

const testPnpmLocalWithScripts = step.dependsOn(allowPnpmBuildScripts)({
  name: "Test pnpm local install deno (with postinstall)",
  run: [
    'TEST_DIR="${{ runner.temp }}/pnpm-test"',
    'mkdir -p "$TEST_DIR"',
    'cd "$TEST_DIR"',
    "npm init -y",
    'EXPECTED_VERSION="deno ${{ steps.publish-verdaccio.outputs.version }}"',
    "pnpm install deno@${{ steps.publish-verdaccio.outputs.version }} --registry http://localhost:4873/",
    'ACTUAL="$(pnpm exec deno -v)"',
    'echo "$ACTUAL"',
    '[ "$ACTUAL" = "$EXPECTED_VERSION" ] || { echo "Version mismatch: expected \'$EXPECTED_VERSION\', got \'$ACTUAL\'"; exit 1; }',
  ],
});

const testPnpmGlobalWithScripts = step.dependsOn(testPnpmLocalWithScripts)({
  name: "Test pnpm global install deno (with postinstall)",
  run: [
    'EXPECTED_VERSION="deno ${{ steps.publish-verdaccio.outputs.version }}"',
    "pnpm install -g deno@${{ steps.publish-verdaccio.outputs.version }} --registry http://localhost:4873/",
    'ACTUAL="$(deno -v)"',
    'echo "$ACTUAL"',
    '[ "$ACTUAL" = "$EXPECTED_VERSION" ] || { echo "Version mismatch: expected \'$EXPECTED_VERSION\', got \'$ACTUAL\'"; exit 1; }',
    "deno eval \"console.log('global pnpm install works')\"",
    "pnpm uninstall -g deno",
  ],
});

const testNpmLocalPowershell = step.dependsOn(testPnpmGlobalWithScripts)({
  name: "Test npm local install deno (PowerShell)",
  if: "runner.os == 'Windows'",
  shell: "pwsh",
  run: [
    'cd "${{ runner.temp }}/npm-test"',
    "npx deno -v",
    "npx deno eval \"console.log('PowerShell npm local install works')\"",
  ],
});

const testNpmGlobalPowershell = step.dependsOn(testNpmLocalPowershell)({
  name: "Test npm global install deno (PowerShell)",
  if: "runner.os == 'Windows'",
  shell: "pwsh",
  run: [
    "npm install -g deno@${{ steps.publish-verdaccio.outputs.version }}",
    "deno -v",
    "deno eval \"console.log('PowerShell npm global install works')\"",
    "npm uninstall -g deno",
  ],
});

const testNpmLocalCmd = step.dependsOn(testNpmGlobalPowershell)({
  name: "Test npm local install deno (cmd)",
  if: "runner.os == 'Windows'",
  shell: "cmd",
  run: [
    'cd "${{ runner.temp }}/npm-test"',
    "npx deno -v",
    "npx deno eval \"console.log('cmd npm local install works')\"",
  ],
});

const testNpmGlobalCmd = step.dependsOn(testNpmLocalCmd)({
  name: "Test npm global install deno (cmd)",
  if: "runner.os == 'Windows'",
  shell: "cmd",
  run: [
    "npm install -g deno@${{ steps.publish-verdaccio.outputs.version }}",
    "deno -v",
    "deno eval \"console.log('cmd npm global install works')\"",
    "npm uninstall -g deno",
  ],
});

const testJob = job("test", {
  name: "npm test (${{ matrix.runner }})",
  needs: [buildJob],
  runsOn: testMatrix.runner,
  timeoutMinutes: 15,
  strategy: {
    failFast: false,
    matrix: testMatrix,
  },
  defaults: {
    run: {
      shell: "bash",
    },
  },
  steps: [testPnpmGlobalWithScripts, testNpmGlobalCmd],
});

// === publish job ===

const publishConfigureGit = step({
  name: "Configure git",
  run: [
    "git config --global core.symlinks true",
    "git config --global fetch.parallel 32",
  ],
});

const publishClone = step.dependsOn(publishConfigureGit)({
  name: "Clone repository",
  uses: "actions/checkout@v6",
  with: { submodules: "recursive" },
});

const publishInstallDeno = step.dependsOn(publishClone)({
  name: "Install Deno",
  uses: "denoland/setup-deno@v2",
  with: { "deno-version": "v2.7.1" },
});

const publishInstallNode = step.dependsOn(publishInstallDeno)({
  name: "Install Node",
  uses: "actions/setup-node@v6",
  with: {
    "node-version": "24.x",
    "registry-url": "https://registry.npmjs.org",
  },
});

const publishDownloadArtifact = step.dependsOn(publishInstallNode)({
  name: "Download npm dist artifact",
  uses: "actions/download-artifact@v6",
  with: {
    name: "npm-dist",
    path: "tools/release/npm",
  },
});

const publishExtractDist = step.dependsOn(publishDownloadArtifact)({
  name: "Extract npm dist",
  run: "tar xf tools/release/npm/dist.tar -C tools/release/npm",
});

const publishToNpm = step.dependsOn(publishExtractDist)({
  name: "Publish to npm",
  run:
    "./tools/release/npm/build.ts ${{ inputs.version || '' }} --publish-only",
});

const workflow = createWorkflow({
  name: "npm_publish",
  on: {
    workflow_dispatch: {
      inputs: {
        version: {
          description: "Version",
          type: "string",
        },
        dry_run: {
          description:
            "Dry run (build and test, but skip publishing to npmjs.org)",
          type: "boolean",
          default: true,
        },
      },
    },
    release: {
      types: ["published"],
    },
  },
  jobs: [
    buildJob,
    testJob,
    {
      id: "publish",
      name: "npm publish",
      needs: [buildJob, testJob],
      if: "!(inputs.dry_run || false)",
      runsOn: "ubuntu-latest",
      timeoutMinutes: 15,
      permissions: {
        "id-token": "write",
      },
      steps: [publishToNpm],
    },
  ],
});

const header = "# GENERATED BY ./npm_publish.ts -- DO NOT DIRECTLY EDIT";

export function generate() {
  return workflow.toYamlString({ header });
}

export const NPM_PUBLISH_YML_URL = new URL(
  "./npm_publish.generated.yml",
  import.meta.url,
);

if (import.meta.main) {
  workflow.writeOrLint({ filePath: NPM_PUBLISH_YML_URL, header });
}
