#!/usr/bin/env -S deno run --reload --allow-run
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import "./unit_tests.ts";
import {
  permissionCombinations,
  Permissions,
  defer,
  newParseUnitTestOutput,
  getProcessPermissions,
  permissionsMatch,
  REGISTERED_UNIT_TESTS
} from "./test_util.ts";

interface TestResult {
  perms: string;
  output?: string;
  result: number;
}

class FileReporter implements Deno.TestReporter {
  private file: Deno.File;
  private encoder: TextEncoder;

  constructor(filename: string) {
    this.file = Deno.openSync(filename, "w+");
    this.encoder = new TextEncoder();
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  async write(msg: any): Promise<void> {
    const encodedMsg = this.encoder.encode(`${JSON.stringify(msg)}\n`);
    await Deno.writeAll(this.file, encodedMsg);
  }

  async start(msg: Deno.StartMsg): Promise<void> {
    await this.write(msg);
  }

  async test(msg: Deno.TestMsg): Promise<void> {
    await this.write(msg);
  }

  async end(msg: Deno.EndMsg): Promise<void> {
    await this.write(msg);
  }

  close(): void {
    this.file.close();
  }
}

async function dropPermissions(
  requiredPermissions: Deno.PermissionName[]
): Promise<void> {
  const permsToDrop: Deno.PermissionName[] = [
    "read",
    "write",
    "net",
    "env",
    "run",
    "plugin",
    "hrtime"
  ];

  for (const flag of requiredPermissions) {
    if (!flag.length) continue;
    const index = permsToDrop.indexOf(flag);
    permsToDrop.splice(index, 1);
  }

  console.log("worker runner dropping permissions: ", permsToDrop);
  for (const perm of permsToDrop) {
    await Deno.permissions.revoke({ name: perm });
  }
}

async function workerRunnerMain(
  filename: string,
  permissions: Deno.PermissionName[]
): Promise<void> {
  // Create reporter, then drop permissions to requested set
  const fileReporter = new FileReporter(filename);
  await dropPermissions(permissions);

  // Register unit tests that match process permissions
  const processPerms = await getProcessPermissions();

  for (const unitTestDefinition of REGISTERED_UNIT_TESTS) {
    if (unitTestDefinition.skip) {
      continue;
    }

    if (!permissionsMatch(processPerms, unitTestDefinition.perms)) {
      continue;
    }

    Deno.test(unitTestDefinition);
  }

  // Permissions dropped we're ready to execute tests
  const results = await Deno.runTests({
    failFast: false,
    exitOnFail: false,
    reporter: fileReporter
  });
  console.log("worker finished running tests", results.stats);
  fileReporter.close();
}

function permsToStrings(perms: Permissions): string[] {
  return Object.keys(perms)
    .map(key => {
      if (!perms[key as keyof Permissions]) return "";

      const cliFlag = key.replace(
        /\.?([A-Z])/g,
        (x, y): string => `-${y.toLowerCase()}`
      );
      return `${cliFlag}`;
    })
    .filter((e): boolean => e.length > 0);
}

function permsToCliFlags(perms: Permissions): string[] {
  return Object.keys(perms)
    .map(key => {
      if (!perms[key as keyof Permissions]) return "";

      const cliFlag = key.replace(
        /\.?([A-Z])/g,
        (x, y): string => `-${y.toLowerCase()}`
      );
      return `--allow-${cliFlag}`;
    })
    .filter((e): boolean => e.length > 0);
}

function fmtPerms(perms: Permissions): string {
  let fmt = permsToCliFlags(perms).join(" ");

  if (!fmt) {
    fmt = "<no permissions>";
  }

  return fmt;
}

async function masterRunnerMain(): Promise<void> {
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
    const permStrs = permsToStrings(perms);

    // TODO: open file
    const r = Math.random() * 100_000;
    const filename = `.report.${~~r}.deno.test`;

    // run subsequent tests using same deno executable
    const args = [
      Deno.execPath(),
      "run",
      "-A",
      "cli/js/tests/unit_test_runner.ts",
      "--",
      "--worker",
      `--filename=${filename}`,
      `--perms=${permStrs.join(",")}`
    ];

    const p = Deno.run({
      args
    });

    // Wait until file is created by subprocess
    while (true) {
      try {
        await Deno.stat(filename);
        break;
      } catch {
        // pass
      }
      await defer(100);
    }

    const reportFile = await Deno.open(filename, "r");
    const { actual, expected, resultOutput } = await newParseUnitTestOutput(
      reportFile,
      true
    );
    reportFile.close();
    Deno.remove(filename);

    let result = 0;

    if (!actual && !expected) {
      console.error("Bad cli/js/tests/unit_test.ts output");
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

// --worker --filename=asdf.txt --perms=net,run,read,env

async function main(): Promise<void> {
  const args = Deno.args.slice(1);
  console.log("worker main", args);
  // return;

  if (args[0] === "--worker") {
    // Worker runner
    const filename = args[1].split("=")[1];
    const perms = args[2].split("=")[1].split(",") as Deno.PermissionName[];
    console.log("test worker", filename, perms);
    await workerRunnerMain(filename, perms);
  } else {
    // Master runner
    await masterRunnerMain();
  }
}

main();
