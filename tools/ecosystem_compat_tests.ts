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
        "120s",
      );
  const duration = Date.now() - state;
  return {
    exitCode: result.code,
    stdout: result.stdout,
    stderr: result.stderr,
    duration,
  };
}

async function testNpm() {
  $.logStep("Testing npm...");
  const report = await runPackageManager("npm", "install");
  if (report.exitCode === 0) {
    $.logStep("npm install succeeded");
  } else {
    $.logWarn(`npm install failed: ${report.stderr}`);
  }
  return report;
}

async function testYarn() {
  $.logStep("Testing yarn...");
  const report = await runPackageManager("yarn", "install");
  if (report.exitCode === 0) {
    $.logStep("yarn install succeeded");
  } else {
    $.logWarn(`yarn install failed: ${report.stderr}`);
  }
  return report;
}

async function testPnpm() {
  $.logStep("Testing pnpm...");
  const report = await runPackageManager("pnpm", "install");
  if (report.exitCode === 0) {
    $.logStep("pnpm install succeeded");
  } else {
    $.logWarn(`pnpm install failed: ${report.stderr}`);
  }
  return report;
}

async function main() {
  $.logStep("Setting up test repo...");
  await setupTestRepo();

  const npmResult = await testNpm();
  const yarnResult = await testYarn();
  const pnpmResult = await testPnpm();
  const reports = {
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
  };

  $.logStep("Final report:");
  $.log(reports);
  await Deno.writeTextFile(
    join(import.meta.dirname!, "ecosystem_report.json"),
    JSON.stringify(reports),
  );
}

try {
  await main();
} finally {
  await teardownTestRepo();
}
