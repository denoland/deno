// Copyright 2018-2025 the Deno authors. MIT license.

import { runSingle } from "./run_all_test_unmodified.ts";
import { assert } from "@std/assert";
import { partition } from "@std/collections/partition";
import { pooledMap } from "@std/async/pool";
import { configFile, type SingleFileConfig } from "./common.ts";

let testSerialId = 0;
export const generateTestSerialId = () => ++testSerialId;

const [sequentialTests, parallelTests] = partition(
  Object.entries(configFile.tests),
  ([testName]) => testName.startsWith("sequential/"),
);

async function run(name: string, testConfig: SingleFileConfig) {
  const result = await runSingle(name, testConfig);
  let msg = "";
  const error = result.error;
  if (error && "message" in error) {
    msg = error.message;
  } else if (error && "stderr" in error) {
    msg = error.stderr;
  } else if (error && "timeout" in error) {
    msg = `Timed out after ${error.timeout}ms`;
  }
  assert(result.result === "pass", `Test "${name}" failed: ${msg}`);
}

function computeIgnores(testConfig: SingleFileConfig): boolean {
  if (testConfig.windows === false && Deno.build.os === "windows") {
    return true;
  } else if (testConfig.linux === false && Deno.build.os === "linux") {
    return true;
  } else if (testConfig.darwin === false && Deno.build.os === "darwin") {
    return true;
  }

  return false;
}

for (const [name, testConfig] of sequentialTests) {
  Deno.test(
    "Node compat: " + name,
    { ignore: computeIgnores(testConfig) },
    async () => {
      await run(name, testConfig);
    },
  );
}

Deno.test("Node compat: parallel tests", async (t) => {
  const iter = pooledMap(
    navigator.hardwareConcurrency,
    parallelTests,
    ([name, testConfig]) =>
      t.step({
        name,
        ignore: computeIgnores(testConfig),
        fn: () => run(name, testConfig),
        sanitizeExit: false,
        sanitizeOps: false,
        sanitizeResources: false,
      }),
  );

  for await (const _ of iter) {
    // Just iterate through the results to ensure all tests are run
  }
});
