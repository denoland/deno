#!/usr/bin/env -S deno run --reload --allow-run
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import "./unit_tests.ts";
import {
  assert,
  readLines,
  permissionCombinations,
  Permissions,
  registerUnitTests,
  SocketReporter,
  fmtPerms
} from "./test_util.ts";

interface PermissionSetTestResult {
  perms: Permissions;
  passed: boolean;
  stats: Deno.TestStats;
  permsStr: string;
  result: number;
}

/**
 * Take a list of permissions and revoke missing permissions.
 */
async function dropWorkerPermissions(
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
  addr: { hostname: string; port: number },
  permissions: Deno.PermissionName[]
): Promise<void> {
  // Create reporter, then drop permissions to requested set
  const conn = await Deno.connect(addr);
  console.log("Test worker online", addr, permissions);
  const socketReporter = new SocketReporter(conn);
  await dropWorkerPermissions(permissions);

  // Register unit tests that match process permissions
  await registerUnitTests();

  // Permissions dropped we're ready to execute tests
  const results = await Deno.runTests({
    failFast: false,
    exitOnFail: false,
    reporter: socketReporter
  });
  console.log("worker finished running tests", results.stats);
  socketReporter.close();
}

function spawnWorkerRunner(addr: string, perms: Permissions): Deno.Process {
  // run subsequent tests using same deno executable
  const permStr = Object.keys(perms)
    .filter((permName): boolean => {
      return perms[permName as Deno.PermissionName] === true;
    })
    .join(",");

  const args = [
    Deno.execPath(),
    "run",
    "-A",
    "cli/js/tests/unit_test_runner.ts",
    "--",
    "--worker",
    `--addr=${addr}`,
    `--perms=${permStr}`
  ];

  const p = Deno.run({
    args
  });

  return p;
}

async function runTestsForPermissionSet(
  reporter: Deno.ConsoleReporter,
  perms: Permissions
): Promise<PermissionSetTestResult> {
  const permsFmt = fmtPerms(perms);
  console.log(`Running tests for: ${permsFmt}`);
  const addr = { hostname: "127.0.0.1", port: 4510 };
  const addrStr = `${addr.hostname}:${addr.port}`;
  const workerListener = Deno.listen(addr);

  const workerProcess = spawnWorkerRunner(addrStr, perms);

  // Wait for worker subprocess to go online
  const conn = await workerListener.accept();

  let err;
  let hasThrown = false;
  let expectedTests;
  let permTestResult;

  try {
    for await (const line of readLines(conn)) {
      const msg = JSON.parse(line);

      if (msg.kind === "start") {
        expectedTests = msg.tests;
        await reporter.start(msg);
        continue;
      }

      if (msg.kind === "test") {
        await reporter.test(msg);
        continue;
      }

      permTestResult = msg.stats;
      await reporter.end(msg);
      break;
    }
  } catch (e) {
    hasThrown = true;
    err = e;
  } finally {
    workerListener.close();
  }

  if (hasThrown) {
    throw err;
  }

  if (typeof expectedTests === "undefined") {
    throw new Error("Worker runner didn't report start");
  }

  if (typeof permTestResult === "undefined") {
    throw new Error("Worker runner didn't report end");
  }

  const workerStatus = await workerProcess.status();
  if (!workerStatus.success) {
    throw new Error(
      `Worker runner exited with status code: ${workerStatus.code}`
    );
  }

  workerProcess.close();

  const actual = permTestResult.passed;
  console.log("expected", expectedTests, "actual", actual);
  const result = expectedTests === actual ? 0 : 1;
  return {
    perms,
    passed: expectedTests === actual,
    permsStr: permsFmt,
    result,
    stats: permTestResult
  };
}

async function masterRunnerMain(): Promise<void> {
  console.log(
    "Discovered permission combinations for tests:",
    permissionCombinations.size
  );

  for (const perms of permissionCombinations.values()) {
    console.log("\t" + fmtPerms(perms));
  }

  const testResults = new Set<PermissionSetTestResult>();
  const consoleReporter = new Deno.ConsoleReporter();

  for (const perms of permissionCombinations.values()) {
    const result = await runTestsForPermissionSet(consoleReporter, perms);
    testResults.add(result);
  }

  // if any run tests returned non-zero status then whole test
  // run should fail
  let testsPassed = true;

  for (const testResult of testResults) {
    const { permsStr, stats } = testResult;
    // TODO: use common functionality from ConsoleReporter
    console.log(`Summary for ${permsStr}`);
    console.log(
      `test result: ${stats.failed ? "FAIL" : "OK"} ` +
        `${stats.passed} passed; ${stats.failed} failed; ` +
        `${stats.ignored} ignored; ${stats.measured} measured; ` +
        `${stats.filtered} filtered out\n`
    );
    testsPassed = testsPassed && testResult.passed;
  }

  if (!testsPassed) {
    console.error("Unit tests failed");
    Deno.exit(1);
  }

  console.log("Unit tests passed");
}

async function main(): Promise<void> {
  // TODO: use `flags` parser
  const args = Deno.args;

  const isWorker = args.includes("--worker");

  if (!isWorker) {
    return await masterRunnerMain();
  }

  const addrArg = args.find(e => e.includes("--addr"));
  assert(typeof addrArg === "string");
  const addrStr = addrArg.split("=")[1];
  const [hostname, port] = addrStr.split(":");
  const addr = { hostname, port: Number(port) };

  let perms: Deno.PermissionName[] = [];
  const permsArg = args.find(e => e.includes("--perms"));
  assert(typeof permsArg === "string");
  const permsStr = permsArg.split("=")[1];
  if (permsStr.length > 0) {
    perms = permsStr.split(",") as Deno.PermissionName[];
  }

  await workerRunnerMain(addr, perms);
}

main();
