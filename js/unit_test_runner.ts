#!/usr/bin/env deno run --reload --allow-run
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import "./unit_tests.ts";
import { permissionCombinations, parseUnitTestOutput } from "./test_util.ts";

interface TestResult {
  perms: string;
  output: string;
  result: number;
}

function permsToCliFlags(perms: Deno.Permissions): string[] {
  return Object.keys(perms)
    .map(
      (key): string => {
        if (!perms[key]) return "";

        const cliFlag = key.replace(
          /\.?([A-Z])/g,
          (x, y): string => `-${y.toLowerCase()}`
        );
        return `--allow-${cliFlag}`;
      }
    )
    .filter((e): boolean => e.length > 0);
}

function fmtPerms(perms: Deno.Permissions): string {
  let fmt = permsToCliFlags(perms).join(" ");

  if (!fmt) {
    fmt = "<no permissions>";
  }

  return fmt;
}

async function main(): Promise<void> {
  console.log(
    "Discovered permission combinations for tests:",
    permissionCombinations.size
  );

  for (const perms of permissionCombinations.values()) {
    console.log("\t" + fmtPerms(perms));
  }

  const testResults = new Set<TestResult>();

  for (const perms of permissionCombinations.values()) {
    const permsFmt = fmtPerms(perms);
    console.log(`Running tests for: ${permsFmt}`);
    const cliPerms = permsToCliFlags(perms);
    // run subsequent tests using same deno executable
    const args = [
      Deno.execPath,
      "run",
      "--no-prompt",
      ...cliPerms,
      "js/unit_tests.ts"
    ];

    const p = Deno.run({
      args,
      stdout: "piped"
    });

    const { actual, expected, resultOutput } = parseUnitTestOutput(
      await p.output(),
      true
    );

    let result = 0;

    if (!actual && !expected) {
      console.error("Bad js/unit_test.ts output");
      result = 1;
    } else if (expected !== actual) {
      result = 1;
    }

    testResults.add({
      perms: permsFmt,
      output: resultOutput,
      result
    });
  }

  // if any run tests returned non-zero status then whole test
  // run should fail
  let testsFailed = false;

  for (const testResult of testResults) {
    console.log(`Summary for ${testResult.perms}`);
    console.log(testResult.output + "\n");
    testsFailed = testsFailed || Boolean(testResult.result);
  }

  if (testsFailed) {
    console.error("Unit tests failed");
    Deno.exit(1);
  }

  console.log("Unit tests passed");
}

main();
