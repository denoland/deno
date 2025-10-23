#!/usr/bin/env -S deno run --allow-all --config=tests/config/deno.json
// Copyright 2018-2025 the Deno authors. MIT license.
import $ from "jsr:@david/dax@^0.42.0";
import { join } from "./util.js";

async function setupTestRepo() {
  await teardownTestRepo();
  await $`git clone https://github.com/lucacasonato/nextjs-demo`;
}

async function teardownTestRepo() {
  await $`rm -rf nextjs-demo`;
}

async function runPackageManager(pm: string, cmd: string) {
  const state = Date.now();
  const result =
    await $`cd nextjs-demo ; rm -rf node_modules ; ${Deno.execPath()} run -A --no-config npm:${pm} ${cmd}`
      .stdout("inheritPiped").stderr("inheritPiped").noThrow().timeout(
        "60s",
      );
  const duration = Date.now() - state;
  return {
    exitCode: result.code,
    stdout: result.stdout,
    stderr: result.stderr,
    duration,
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
  $.logStep("Setting up test repo...");
  await setupTestRepo();
  $.logStep("Testing npm...");
  const npmResult = await testNpm();
  if (npmResult.exitCode !== 0) {
    passed = false;
    $.logWarn(`npm install failed: ${npmResult.stderr}`);
  }
  $.logStep("Testing yarn...");
  const yarnResult = await testYarn();
  if (yarnResult.exitCode !== 0) {
    passed = false;
    $.logWarn(`yarn install failed: ${yarnResult.stderr}`);
  }
  $.logStep("Testing pnpm...");
  const pnpmResult = await testPnpm();
  if (pnpmResult.exitCode !== 0) {
    passed = false;
    $.logWarn(`pnpm install failed: ${pnpmResult.stderr}`);
  }
  if (passed) {
    $.logStep("All tests passed!");
  } else {
    $.logError("Some tests failed");
  }
  await Deno.writeTextFile(
    join(import.meta.dirname!, "ecosystem_report.json"),
    JSON.stringify({
      npm: {
        exitCode: npmResult.exitCode,
        duration: npmResult.duration,
      },
      yarn: {
        exitCode: yarnResult.exitCode,
        duration: yarnResult.duration,
      },
      pnpm: {
        exitCode: pnpmResult.exitCode,
        duration: pnpmResult.duration,
      },
    }),
  );
}

try {
  await main();
} finally {
  await teardownTestRepo();
}
