#!/usr/bin/env deno run --reload --allow-run
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import "./unit_tests.ts";
import { permissionCombinations } from "./test_util.ts";

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

function parseUnitTestOutput(rawOutput: Uint8Array, print: boolean) {
  const decoder = new TextDecoder();
  const output = decoder.decode(rawOutput);

  let expected = null,
    actual = null,
    result = null;

  for (const line of output.split("\n")) {
    if (!expected) {
      // expect "running 30 tests"
      const match = line.match(/running (\d+) tests/);
      expected = Number.parseInt(match[1]);
    } else if (line.indexOf("test result:") !== -1) {
      result = line;
    }

    if (print) {
      console.log(line);
    }
  }

  // Check that the number of expected tests equals what was reported at the
  // bottom.
  if (result) {
    // result should be a string like this:
    // "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ..."
    const match = result.match(/(\d+) passed/);
    actual = Number.parseInt(match[1]);
  }

  return { actual, expected };
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
