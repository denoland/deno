#!/usr/bin/env deno --allow-run
// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// This test is executed as part of tools/test.py
// But it can also be run manually: ./target/debug/deno js/run_unit_tests.ts
// You must have ./tools/http_server.py running in the background if you run
// this manually.

import { args, exit, run } from "deno";

function extractNumber(pattern: RegExp, str: string) {
  const matches = pattern.exec(str);
  if (matches == null || pattern.exec(str) != null) {
    return;
  }
  return Number(matches[1]);
}

function parseUnitTestOutput(
  output: string
): [string | undefined, string | undefined] {
  let expected;
  let actual;
  let result;
  for (const line of output.split("\n")) {
    if (expected == null) {
      // expect "running 30 tests"
      expected = extractNumber(/running (\d+) tests/g, line);
    } else if (line.includes("test result:")) {
      result = line;
    }
    console.log(line);
  }
  // Check that the number of expected tests equals what was reported at the
  // bottom.
  if (result) {
    // result should be a string like this:
    // "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ..."
    actual = extractNumber(/(\d+) passed/g, result);
  }
  return [actual, expected];
}

async function runUnitTest2(args: string[]) {
  const process = run({
    args,
    stdin: "null",
    stdout: "piped"
  });
  const outputBuf = await process.output();
  const output = new TextDecoder("utf-8").decode(outputBuf);

  const { code: errorCode } = await process.status();
  if (errorCode !== 0) {
    console.log(output);
    exit(errorCode);
  }
  const [actual, expected] = parseUnitTestOutput(output);
  if (actual == null && expected == null) {
    throw new Error("Bad js/unit_test.ts output");
  }
  if (expected !== actual) {
    console.log("expected", expected, "actual", actual);
    throw new Error("expected tests did not equal actual");
  }
}

function runUnitTest(denoExe: string, permStr: string, flags: string[] = []) {
  const cmd = [denoExe, "--reload", "js/unit_tests.ts", permStr].concat(flags);
  return runUnitTest2(cmd);
}

async function unitTests(denoExe: string) {
  await runUnitTest(denoExe, "permW0N0E0R0");
  await runUnitTest(denoExe, "permW1N0E0R0", ["--allow-write"]);
  await runUnitTest(denoExe, "permW0N1E0R0", ["--allow-net"]);
  await runUnitTest(denoExe, "permW0N0E1R0", ["--allow-env"]);
  await runUnitTest(denoExe, "permW0N0E0R1", ["--allow-run"]);
  await runUnitTest(denoExe, "permW1N0E0R1", ["--allow-run", "--allow-write"]);

  // TODO We might accidentally miss some. We should be smarter about which we
  // run. Maybe we can use the "filtered out" number to check this.

  // These are not strictly unit tests for Deno, but for ts_library_builder.
  // They run under Node, but use the same //js/testing/ library.
  await runUnitTest2([
    "node",
    "./node_modules/.bin/ts-node",
    "--project",
    "tools/ts_library_builder/tsconfig.json",
    "tools/ts_library_builder/test.ts"
  ]);
}

function main() {
  if (args.length < 2) {
    console.log("Usage deno --allow-run ./js/unit_tests.ts target/debug/deno");
    exit(1);
  }

  unitTests(args[1]);
}

main();
