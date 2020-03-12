#!/usr/bin/env -S deno run --reload --allow-run
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { readLines } from "../../../std/io/bufio.ts";
import "./unit_tests.ts";
import {
  permissionCombinations,
  Permissions,
  registerUnitTests
} from "./test_util.ts";
import { assert } from "../../../std/testing/asserts.ts";

interface TestResult {
  perms: string;
  output?: string;
  result: number;
}

class SocketReporter implements Deno.TestReporter {
  private encoder: TextEncoder;

  constructor(private conn: Deno.Conn) {
    this.encoder = new TextEncoder();
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  async write(msg: any): Promise<void> {
    const encodedMsg = this.encoder.encode(`${JSON.stringify(msg)}\n`);
    await Deno.writeAll(this.conn, encodedMsg);
  }

  async start(msg: Deno.StartMsg): Promise<void> {
    await this.write(msg);
  }

  async test(msg: Deno.TestMsg): Promise<void> {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const serializedMsg: any = { ...msg };
    delete serializedMsg.result.fn;

    if (serializedMsg.result.error) {
      serializedMsg.result.error = String(serializedMsg.result.error.stack);
    }

    await this.write(serializedMsg);
  }

  async end(msg: Deno.EndMsg): Promise<void> {
    await this.write(msg);
  }

  close(): void {
    this.conn.close();
  }
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
  // const fileReporter = new FileReporter(filename);
  const conn = await Deno.connect(addr);
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

function spawnWorkerRunner(addr: string, perms: Permissions): Deno.Process {
  const permStrs = permsToStrings(perms);

  // run subsequent tests using same deno executable
  const args = [
    Deno.execPath(),
    "run",
    "-A",
    "cli/js/tests/unit_test_runner.ts",
    "--",
    "--worker",
    `--addr=${addr}`,
    `--perms=${permStrs.join(",")}`
  ];

  const p = Deno.run({
    args
  });

  return p;
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
  const consoleReporter = new Deno.ConsoleReporter();

  for (const perms of permissionCombinations.values()) {
    const permsFmt = fmtPerms(perms);
    console.log(`Running tests for: ${permsFmt}`);

    const addr = { hostname: "127.0.0.1", port: 4510 };
    const addrStr = `${addr.hostname}:${addr.port}`;
    const workerListener = Deno.listen(addr);

    const workerProcess = spawnWorkerRunner(addrStr, perms);

    // Wait for worker subprocess to go online
    const conn = await workerListener.accept();
    const linesIterator = readLines(conn);

    let err;
    let hasThrown = false;
    let expectedTests;
    let permTestResult;

    try {
      const { value } = await linesIterator.next();
      const msg = JSON.parse(value);
      assert(msg.kind === "start");
      expectedTests = msg.tests;
      consoleReporter.start(msg);

      for await (const line of readLines(conn)) {
        const msg = JSON.parse(line);

        if (msg.kind === "test") {
          await consoleReporter.test(msg);
          continue;
        }

        permTestResult = msg.stats;
        await consoleReporter.end(msg);
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

    testResults.add({
      perms: permsFmt,
      output: "",
      result
    });
  }

  // if any run tests returned non-zero status then whole test
  // run should fail
  let testsFailed = false;

  for (const testResult of testResults) {
    console.log(`Summary for ${testResult.perms} ${testResult.result}`);
    console.log(testResult.output + "\n");
    testsFailed = testsFailed || Boolean(testResult.result);
  }

  if (testsFailed) {
    console.error("Unit tests failed");
    Deno.exit(1);
  }

  console.log("Unit tests passed");
}

async function main(): Promise<void> {
  const args = Deno.args.slice(1);
  console.log("worker main", args);

  // TODO: use `flags` parser
  if (args[0] === "--worker") {
    // Worker runner
    const addr = args[1].split("=")[1];
    const perms = args[2].split("=")[1].split(",") as Deno.PermissionName[];
    console.log("test worker", addr, perms);
    const [hostname, port] = addr.split(":");
    await workerRunnerMain({ hostname, port: Number(port) }, perms);
  } else {
    // Master runner
    await masterRunnerMain();
  }
}

main();
