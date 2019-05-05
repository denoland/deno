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
          (x, y) => `-${y.toLowerCase()}`
        );
        return `--allow-${cliFlag}`;
      }
    )
    .filter(e => e.length);
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
    const args = [Deno.execPath, "run", ...cliPerms, "js/unit_tests.ts"];

    const p = Deno.run({
      args,
      stdout: "inherit"
    });

    const { code } = await p.status();
    testResults.push(code);
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
