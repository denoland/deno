// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { magenta } from "std/fmt/colors.ts";
import { dirname, fromFileUrl, join } from "std/path/mod.ts";
import { fail } from "std/testing/asserts.ts";
import {
  config,
  getPathsFromTestSuites,
  partitionParallelTestPaths,
} from "./common.ts";

// If the test case is invoked like
// deno test -A cli/tests/node_compat/test.ts -- <test-names>
// Use the test-names as filters
const filters = Deno.args;
const hasFilters = filters.length > 0;

/**
 * This script will run the test files specified in the configuration file
 *
 * Each test file will be run independently and wait until completion, if an abnormal
 * code for the test is reported, the test suite will fail immediately
 */

const toolsPath = dirname(fromFileUrl(import.meta.url));
const stdRootUrl = new URL("../../", import.meta.url).href;
const testPaths = partitionParallelTestPaths(
  getPathsFromTestSuites(config.tests),
);
const cwd = new URL(".", import.meta.url);
const importMap = "import_map.json";
const windowsIgnorePaths = new Set(
  getPathsFromTestSuites(config.windowsIgnore),
);
const darwinIgnorePaths = new Set(
  getPathsFromTestSuites(config.darwinIgnore),
);

const decoder = new TextDecoder();
Deno.test("Node.js compatibility", async (t) => {
  let testSerialId = 0;
  let shouldBlock = true;
  const pending = [];
  // Parallelizable tests are partitioned to the end so that we can simply stop
  // blocking on tests when we get there.
  for (const paths of [testPaths.sequential, testPaths.parallel]) {
    for (const path of paths) {
      // If filter patterns are given and any pattern doesn't match
      // to the file path, then skip the case
      if (
        filters.length > 0 &&
        filters.every((pattern) => !path.includes(pattern))
      ) {
        continue;
      }
      const isTodo = path.includes("TODO");
      const ignore =
        (Deno.build.os === "windows" && windowsIgnorePaths.has(path)) ||
        (Deno.build.os === "darwin" && darwinIgnorePaths.has(path)) || isTodo;
      const handle = t.step({
        name: `Node.js compatibility "${path}"`,
        ignore,
        sanitizeOps: false,
        sanitizeResources: false,
        sanitizeExit: false,
        fn: async () => {
          const testCase = join(toolsPath, "test", path);

          const v8Flags = ["--stack-size=4000"];
          const testSource = await Deno.readTextFile(testCase);
          // TODO(kt3k): Parse `Flags` directive correctly
          if (testSource.includes("Flags: --expose_externalize_string")) {
            v8Flags.push("--expose-externalize-string");
          }

          const args = [
            "run",
            "-A",
            "--quiet",
            "--unstable",
            //"--unsafely-ignore-certificate-errors",
            "--v8-flags=" + v8Flags.join(),
            testCase.endsWith(".mjs")
              ? "--import-map=" + importMap
              : "runner.ts",
            testCase,
          ];

          // Pipe stdout in order to output each test result as Deno.test output
          // That way the tests will respect the `--quiet` option when provided
          const command = new Deno.Command(Deno.execPath(), {
            args,
            env: {
              DENO_NODE_COMPAT_URL: stdRootUrl,
              TEST_SERIAL_ID: String(testSerialId++),
            },
            cwd,
          });
          const { code, stdout, stderr } = await command.output();

          if (code !== 0) {
            // If the test case failed, show the stdout, stderr, and instruction
            // for repeating the single test case.
            if (stdout.length) console.log(decoder.decode(stdout));
            console.log(`Error: "${path}" failed`);
            console.log(
              "You can repeat only this test with the command:",
              magenta(
                `./target/debug/deno test -A --import-map cli/tests/node_compat/import_map.json cli/tests/node_compat/test.ts -- ${path}`,
              ),
            );
            fail(decoder.decode(stderr));
          } else if (hasFilters) {
            // Even if the test case is successful, shows the stdout and stderr
            // when test case filtering is specified.
            if (stdout.length) console.log(decoder.decode(stdout));
            if (stderr.length) console.log(decoder.decode(stderr));
          }
        },
      });
      if (shouldBlock) {
        await handle;
      } else {
        pending.push(handle);
      }
    }
    shouldBlock = false;
  }
  await Promise.all(pending);
});

function checkConfigTestFilesOrder(testFileLists: Array<string[]>) {
  for (let testFileList of testFileLists) {
    testFileList = testFileList.filter((name) => !name.startsWith("TODO:"));
    const sortedTestList = JSON.parse(JSON.stringify(testFileList));
    sortedTestList.sort();
    if (JSON.stringify(testFileList) !== JSON.stringify(sortedTestList)) {
      throw new Error(
        `File names in \`config.json\` are not correct order.`,
      );
    }
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
