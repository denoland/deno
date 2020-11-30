#!/usr/bin/env -S deno run --reload --allow-run
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  colors,
  fmtPerms,
  parseArgs,
  permissionCombinations,
  Permissions,
  readLines,
  REGISTERED_UNIT_TESTS,
  registerUnitTests,
  reportToConn,
} from "./test_util.ts";
import "./unit_tests.ts";

// @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
const internalObj = Deno[Deno.internal];
// deno-lint-ignore no-explicit-any
const reportToConsole = internalObj.reportToConsole as (message: any) => void;
// deno-lint-ignore no-explicit-any
const runTests = internalObj.runTests as (options: any) => Promise<any>;

interface PermissionSetTestResult {
  perms: Permissions;
  passed: boolean;
  // deno-lint-ignore no-explicit-any
  endMessage: any;
  permsStr: string;
}

const PERMISSIONS: Deno.PermissionName[] = [
  "read",
  "write",
  "net",
  "env",
  "run",
  "plugin",
  "hrtime",
];

/**
 * Take a list of permissions and revoke missing permissions.
 */
async function dropWorkerPermissions(
  requiredPermissions: Deno.PermissionName[],
): Promise<void> {
  const permsToDrop = PERMISSIONS.filter((p): boolean => {
    return !requiredPermissions.includes(p);
  });

  for (const perm of permsToDrop) {
    await Deno.permissions.revoke({ name: perm });
  }
}

async function workerRunnerMain(
  addrStr: string,
  permsStr: string,
  filter?: string,
): Promise<void> {
  const [hostname, port] = addrStr.split(":");
  const addr = { hostname, port: Number(port) };

  let perms: Deno.PermissionName[] = [];
  if (permsStr.length > 0) {
    perms = permsStr.split(",") as Deno.PermissionName[];
  }
  // Setup reporter
  const conn = await Deno.connect(addr);
  // Drop current process permissions to requested set
  await dropWorkerPermissions(perms);
  // Register unit tests that match process permissions
  await registerUnitTests();
  // Execute tests
  await runTests({
    exitOnFail: false,
    filter,
    reportToConsole: false,
    onMessage: reportToConn.bind(null, conn),
  });
}

function spawnWorkerRunner(
  verbose: boolean,
  addr: string,
  perms: Permissions,
  filter?: string,
): Deno.Process {
  // run subsequent tests using same deno executable
  const permStr = Object.keys(perms)
    .filter((permName): boolean => {
      return perms[permName as Deno.PermissionName] === true;
    })
    .join(",");

  const cmd = [
    Deno.execPath(),
    "run",
    "--unstable", // TODO(ry) be able to test stable vs unstable
    "-A",
    "cli/tests/unit/unit_test_runner.ts",
    "--worker",
    `--addr=${addr}`,
    `--perms=${permStr}`,
  ];

  if (filter) {
    cmd.push("--");
    cmd.push(filter);
  }

  const ioMode = verbose ? "inherit" : "null";

  const p = Deno.run({
    cmd,
    stdin: ioMode,
    stdout: ioMode,
    stderr: ioMode,
  });

  return p;
}

async function runTestsForPermissionSet(
  listener: Deno.Listener,
  addrStr: string,
  verbose: boolean,
  perms: Permissions,
  filter?: string,
): Promise<PermissionSetTestResult> {
  const permsFmt = fmtPerms(perms);
  console.log(`Running tests for: ${permsFmt}`);
  const workerProcess = spawnWorkerRunner(verbose, addrStr, perms, filter);
  // Wait for worker subprocess to go online
  const conn = await listener.accept();

  let expectedPassedTests;
  // deno-lint-ignore no-explicit-any
  let endMessage: any;

  try {
    for await (const line of readLines(conn)) {
      // deno-lint-ignore no-explicit-any
      const message = JSON.parse(line) as any;
      reportToConsole(message);
      if (message.start != null) {
        expectedPassedTests = message.start.tests.length;
      } else if (message.end != null) {
        endMessage = message.end;
      }
    }
  } finally {
    // Close socket to worker.
    conn.close();
  }

  if (expectedPassedTests == null) {
    throw new Error("Worker runner didn't report start");
  }

  if (endMessage == null) {
    throw new Error("Worker runner didn't report end");
  }

  const workerStatus = await workerProcess.status();
  if (!workerStatus.success) {
    throw new Error(
      `Worker runner exited with status code: ${workerStatus.code}`,
    );
  }

  workerProcess.close();

  const passed = expectedPassedTests === endMessage.passed + endMessage.ignored;

  return {
    perms,
    passed,
    permsStr: permsFmt,
    endMessage,
  };
}

async function masterRunnerMain(
  verbose: boolean,
  filter?: string,
): Promise<void> {
  console.log(
    "Discovered permission combinations for tests:",
    permissionCombinations.size,
  );

  for (const perms of permissionCombinations.values()) {
    console.log("\t" + fmtPerms(perms));
  }

  const testResults = new Set<PermissionSetTestResult>();
  const addr = { hostname: "127.0.0.1", port: 4510 };
  const addrStr = `${addr.hostname}:${addr.port}`;
  const listener = Deno.listen(addr);

  for (const perms of permissionCombinations.values()) {
    const result = await runTestsForPermissionSet(
      listener,
      addrStr,
      verbose,
      perms,
      filter,
    );
    testResults.add(result);
  }

  // if any run tests returned non-zero status then whole test
  // run should fail
  let testsPassed = true;

  for (const testResult of testResults) {
    const { permsStr, endMessage } = testResult;
    console.log(`Summary for ${permsStr}`);
    reportToConsole({ end: endMessage });
    testsPassed = testsPassed && testResult.passed;
  }

  if (!testsPassed) {
    console.error("Unit tests failed");
    Deno.exit(1);
  }

  console.log("Unit tests passed");

  if (REGISTERED_UNIT_TESTS.find(({ only }) => only)) {
    console.error(
      `\n${colors.red("FAILED")} because the "only" option was used`,
    );
    Deno.exit(1);
  }
}

const HELP = `Unit test runner

Run tests matching current process permissions:

  deno --allow-write unit_test_runner.ts

  deno --allow-net --allow-hrtime unit_test_runner.ts

  deno --allow-write unit_test_runner.ts -- testWriteFile

Run "master" process that creates "worker" processes
for each discovered permission combination:

  deno -A unit_test_runner.ts --master

Run worker process for given permissions:

  deno -A unit_test_runner.ts --worker --perms=net,read,write --addr=127.0.0.1:4500


OPTIONS:
  --master
    Run in master mode, spawning worker processes for
    each discovered permission combination

  --worker
    Run in worker mode, requires "perms" and "addr" flags,
    should be run with "-A" flag; after setup worker will
    drop permissions to required set specified in "perms"

  --perms=<perm_name>...
    Set of permissions this process should run tests with,

  --addr=<addr>
    Address of TCP socket for reporting

ARGS:
  -- <filter>...
    Run only tests with names matching filter, must
    be used after "--"
`;

function assertOrHelp(expr: unknown): asserts expr {
  if (!expr) {
    console.log(HELP);
    Deno.exit(1);
  }
}

async function main(): Promise<void> {
  const args = parseArgs(Deno.args, {
    boolean: ["master", "worker", "verbose"],
    "--": true,
  });

  if (args.help) {
    console.log(HELP);
    return;
  }

  const filter = args["--"][0];

  // Master mode
  if (args.master) {
    return masterRunnerMain(args.verbose, filter);
  }

  // Worker mode
  if (args.worker) {
    assertOrHelp(typeof args.addr === "string");
    assertOrHelp(typeof args.perms === "string");
    return workerRunnerMain(args.addr, args.perms, filter);
  }

  // Running tests matching current process permissions
  await registerUnitTests();
  await runTests({ filter });
}

main();
