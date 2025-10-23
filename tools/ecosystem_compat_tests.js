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
  await $`cd nextjs-demo && rm -rf node_modules && ${Deno.execPath()} run -A --no-config npm:${pm} ${cmd}`
    .timeout(
      "60s",
    );
}

async function testNpm() {
  await runPackageManager("npm", "install");
}

async function testYarn() {
  await runPackageManager("yarn", "install");
}

async function testPnpm() {
  await runPackageManager("pnpm", "install");
}

async function main() {
  $.log("Setting up test repo...");
  await setupTestRepo();
  $.log("Testing npm...");
  await testNpm();
  $.log("Testing yarn...");
  await testYarn();
  $.log("Testing pnpm...");
  await testPnpm();
  $.log("All tests passed!");
}

try {
  await main();
} finally {
  await teardownTestRepo();
}
