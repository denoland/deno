// Copyright 2018-2025 the Deno authors. MIT license.

import { parse } from "@std/toml";
import { runSingle } from "./run_all_test_unmodified.ts";
import { assert } from "@std/assert";
import { partition } from "@std/collections/partition";
import { pooledMap } from "@std/async/pool";

let testSerialId = 0;
export const generateTestSerialId = () => ++testSerialId;

interface SingleFileConfig {
  flaky?: boolean;
}

type Config = {
  tests: Record<string, SingleFileConfig>;
};

const configFile = await Deno.readTextFile(
  new URL("./config.toml", import.meta.url),
).then(parse);

const [sequentialTests, parallelTests] = partition(
  Object.entries((configFile as Config).tests),
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

for (const [name, testConfig] of sequentialTests) {
  Deno.test("Node compat: " + name, async () => {
    await run(name, testConfig);
  });
}

Deno.test("Node compat: parallel tests", async (t) => {
  const iter = pooledMap(
    navigator.hardwareConcurrency,
    parallelTests,
    ([name, testConfig]) =>
      t.step({
        name,
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
