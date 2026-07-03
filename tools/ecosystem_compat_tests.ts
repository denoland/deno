#!/usr/bin/env -S deno run --allow-all --config=tests/config/deno.json
// Copyright 2018-2026 the Deno authors. MIT license.
import $ from "jsr:@david/dax@^0.42.0";
import { join } from "./util.js";

async function setupTestRepo() {
  await teardownTestRepo();
  await $`git clone https://github.com/lucacasonato/nextjs-demo`;
}

async function teardownTestRepo() {
  await $`rm -rf nextjs-demo`;
}

async function runPackageManager(pm: string, args: string[]) {
  const state = Date.now();
  // pnpm's bundled upath2 reads `who.__proto__.constructor`, which Deno
  // exposes only behind `--unstable-unsafe-proto`.
  const denoFlags = pm === "pnpm" ? ["--unstable-unsafe-proto"] : [];
  const result =
    await $`cd nextjs-demo ; rm -rf node_modules ; ${Deno.execPath()} run -A ${denoFlags} --no-config npm:${pm} ${args}`
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
  const report = await runPackageManager("npm", ["install"]);
  if (report.exitCode === 0) {
    $.logStep("npm install succeeded");
  } else {
    $.logWarn(`npm install failed: ${report.stderr}`);
  }
  return report;
}

async function testYarn() {
  $.logStep("Testing yarn...");
  const report = await runPackageManager("yarn", ["install"]);
  if (report.exitCode === 0) {
    $.logStep("yarn install succeeded");
  } else {
    $.logWarn(`yarn install failed: ${report.stderr}`);
  }
  return report;
}

async function testPnpm() {
  $.logStep("Testing pnpm...");
  // pnpm 11's strict-builds policy exits 1 with `ERR_PNPM_IGNORED_BUILDS`
  // in non-interactive mode whenever a dependency has an unapproved
  // postinstall script (e.g. `unrs-resolver`); allow them all so the
  // smoke test reflects whether the install actually works.
  const report = await runPackageManager("pnpm", [
    "install",
    "--config.dangerouslyAllowAllBuilds=true",
  ]);
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
