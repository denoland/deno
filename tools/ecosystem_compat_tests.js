#!/usr/bin/env -S deno run --allow-all --config=tests/config/deno.json
// Copyright 2018-2025 the Deno authors. MIT license.
import $ from "jsr:@david/dax@^0.42.0";

async function setupTestRepo() {
  await teardownTestRepo();
  await $`git clone https://github.com/lucacasonato/nextjs-demo`;
}

async function teardownTestRepo() {
  await $`rm -rf nextjs-demo`;
}

async function runPackageManager(pm, cmd) {
  const result =
    await $`cd nextjs-demo ; rm -rf node_modules ; ${Deno.execPath()} run -A --no-config npm:${pm} ${cmd}`
      .noThrow().timeout(
        "60s",
      );

  return {
    exitCode: result.exitCode,
    stdout: result.stdout,
    stderr: result.stderr,
  };
}

function testNpm() {
  return runPackageManager("npm", "install");
}

function testYarn() {
  return runPackageManager("yarn", "install");
}

function testPnpm() {
  return runPackageManager("pnpm", "install");
}

async function main() {
  let passed = true;
  $.log("Setting up test repo...");
  await setupTestRepo();
  $.log("Testing npm...");
  const npmResult = await testNpm();
  if (npmResult.exitCode !== 0) {
    passed = false;
    $.log(`npm install failed: ${npmResult.stderr}`);
  }
  $.log("Testing yarn...");
  const yarnResult = await testYarn();
  if (yarnResult.exitCode !== 0) {
    passed = false;
    $.log(`yarn install failed: ${yarnResult.stderr}`);
  }
  $.log("Testing pnpm...");
  const pnpmResult = await testPnpm();
  if (pnpmResult.exitCode !== 0) {
    passed = false;
    $.log(`pnpm install failed: ${pnpmResult.stderr}`);
  }
  if (!passed) {
    throw new Error("Some tests failed");
  }

  $.log("All tests passed!");
}

try {
  await main();
} finally {
  await teardownTestRepo();
}
