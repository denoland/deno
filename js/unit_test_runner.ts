#!/usr/bin/env deno run --reload --allow-run
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import "./unit_tests.ts";
import { permissionCombinations, parseUnitTestOutput } from "./test_util.ts";

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

  const testResults = [];

  for (const perms of permissionCombinations.values()) {
    console.log(`Running tests for: ${fmtPerms(perms)}`);
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

    await p.status();
    const { actual, expected } = parseUnitTestOutput(await p.output(), true);

    if (!actual && !expected) {
      console.error("Bad js/unit_test.ts output");
      testResults.push(1);
    } else if (expected !== actual) {
      testResults.push(1);
    } else {
      testResults.push(0);
    }
  }

  // if any run tests returned non-zero status then whole test
  // run should fail
  const testsFailed = testResults.some(
    (result): boolean => {
      return result !== 0;
    }
  );

  if (testsFailed) {
    Deno.exit(1);
  }
}

main();
