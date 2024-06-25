// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/**
 * This script will run the test files specified in the configuration file.
 *
 * Each test file will be run independently (in a separate process as this is
 * what Node.js is doing) and we wait until it completes. If the process reports
 * an abnormal code, the test is reported and the test suite will fail
 * immediately.
 *
 * Some tests check for presence of certain `process.exitCode`.
 * Some tests depends on directories/files created by other tests - they must
 * all share the same working directory.
 */

import { magenta } from "@std/fmt/colors.ts";
import { pooledMap } from "@std/async/pool.ts";
import { dirname, fromFileUrl, join } from "@std/path/mod.ts";
import { assertEquals, fail } from "@std/assert/mod.ts";
import {
  config,
  getPathsFromTestSuites,
  partitionParallelTestPaths,
} from "./common.ts";

// If the test case is invoked like
// deno test -A tests/node_compat/test.ts -- <test-names>
// Use the <test-names> as filters
const filters = Deno.args;
const hasFilters = filters.length > 0;
const toolsPath = dirname(fromFileUrl(import.meta.url));
const testPaths = partitionParallelTestPaths(
  getPathsFromTestSuites(config.tests),
);
const cwd = new URL(".", import.meta.url);
const windowsIgnorePaths = new Set(
  getPathsFromTestSuites(config.windowsIgnore),
);
const darwinIgnorePaths = new Set(
  getPathsFromTestSuites(config.darwinIgnore),
);

const decoder = new TextDecoder();
let testSerialId = 0;

async function runTest(t: Deno.TestContext, path: string): Promise<void> {
  // If filter patterns are given and any pattern doesn't match
  // to the file path, then skip the case
  if (
    filters.length > 0 &&
    filters.every((pattern) => !path.includes(pattern))
  ) {
    return;
  }
  const ignore =
    (Deno.build.os === "windows" && windowsIgnorePaths.has(path)) ||
    (Deno.build.os === "darwin" && darwinIgnorePaths.has(path));
  await t.step({
    name: `Node.js compatibility "${path}"`,
    ignore,
    sanitizeOps: false,
    sanitizeResources: false,
    sanitizeExit: false,
    fn: async () => {
      const testCase = join(toolsPath, "test", path);

      const v8Flags = ["--stack-size=4000"];
      const testSource = await Deno.readTextFile(testCase);
      const envVars: Record<string, string> = {};
      // TODO(kt3k): Parse `Flags` directive correctly
      if (testSource.includes("Flags: --expose_externalize_string")) {
        v8Flags.push("--expose-externalize-string");
        // TODO(bartlomieju): disable verifying globals if that V8 flag is
        // present. Even though we should be able to pass a list of globals
        // that are allowed, it doesn't work, because the list is expected to
        // contain actual JS objects, not strings :)).
        envVars["NODE_TEST_KNOWN_GLOBALS"] = "0";
      }
      // TODO(nathanwhit): once we match node's behavior on executing
      // `node:test` tests when we run a file, we can remove this
      const usesNodeTest = testSource.includes("node:test");
      const args = [
        usesNodeTest ? "test" : "run",
        "-A",
        "--quiet",
        //"--unsafely-ignore-certificate-errors",
        "--unstable-unsafe-proto",
        "--unstable-bare-node-builtins",
        "--v8-flags=" + v8Flags.join(),
      ];
      if (usesNodeTest) {
        // deno test typechecks by default + we want to pass script args
        args.push("--no-check", "runner.ts", "--", testCase);
      } else {
        args.push("runner.ts", testCase);
      }

      // Pipe stdout in order to output each test result as Deno.test output
      // That way the tests will respect the `--quiet` option when provided
      const command = new Deno.Command(Deno.execPath(), {
        args,
        env: {
          TEST_SERIAL_ID: String(testSerialId++),
          ...envVars,
        },
        cwd,
        stdout: "piped",
        stderr: "piped",
      }).spawn();
      const warner = setTimeout(() => {
        console.error(`Test is running slow: ${testCase}`);
      }, 2 * 60_000);
      const killer = setTimeout(() => {
        console.error(
          `Test ran far too long, terminating with extreme prejudice: ${testCase}`,
        );
        command.kill();
      }, 10 * 60_000);
      const { code, stdout, stderr } = await command.output();
      clearTimeout(warner);
      clearTimeout(killer);

      if (code !== 0) {
        // If the test case failed, show the stdout, stderr, and instruction
        // for repeating the single test case.
        if (stdout.length) {
          console.log(decoder.decode(stdout));
        }
        const stderrOutput = decoder.decode(stderr);
        const repeatCmd = magenta(
          `./target/debug/deno test --config tests/config/deno.json -A tests/node_compat/test.ts -- ${path}`,
        );
        const msg = `"${magenta(path)}" failed:

${stderrOutput}

You can repeat only this test with the command:

  ${repeatCmd}
`;
        console.log(msg);
        fail(msg);
      } else if (hasFilters) {
        // Even if the test case is successful, shows the stdout and stderr
        // when test case filtering is specified.
        if (stdout.length) console.log(decoder.decode(stdout));
        if (stderr.length) console.log(decoder.decode(stderr));
      }
    },
  });
}

Deno.test("Node.js compatibility", async (t) => {
  for (const path of testPaths.sequential) {
    await runTest(t, path);
  }
  const testPool = pooledMap(
    navigator.hardwareConcurrency,
    testPaths.parallel,
    (path) => runTest(t, path),
  );
  const testCases = [];
  for await (const testCase of testPool) {
    testCases.push(testCase);
  }
  await Promise.all(testCases);
});

function checkConfigTestFilesOrder(testFileLists: Array<string[]>) {
  for (const testFileList of testFileLists) {
    const sortedTestList = JSON.parse(JSON.stringify(testFileList));
    sortedTestList.sort((a: string, b: string) =>
      a.toLowerCase().localeCompare(b.toLowerCase())
    );
    assertEquals(
      testFileList,
      sortedTestList,
      "File names in `config.json` are not correct order.",
    );
  }
}

if (!hasFilters) {
  Deno.test("checkConfigTestFilesOrder", function () {
    checkConfigTestFilesOrder([
      ...Object.keys(config.ignore).map((suite) => config.ignore[suite]),
      ...Object.keys(config.tests).map((suite) => config.tests[suite]),
    ]);
  });
}
