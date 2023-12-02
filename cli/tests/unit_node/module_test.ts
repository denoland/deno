// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { createRequire, Module } from "node:module";
import { assert, assertEquals } from "../../../test_util/std/assert/mod.ts";
import process from "node:process";
import * as path from "node:path";

Deno.test("[node/module _preloadModules] has internal require hook", () => {
  // Check if it's there
  // deno-lint-ignore no-explicit-any
  (Module as any)._preloadModules([
    "./cli/tests/unit_node/testdata/add_global_property.js",
  ]);
  // deno-lint-ignore no-explicit-any
  assertEquals((globalThis as any).foo, "Hello");
});

Deno.test("[node/module runMain] loads module using the current process.argv", () => {
  process.argv = [
    process.argv[0],
    "./cli/tests/unit_node/testdata/add_global_property_run_main.js",
  ];

  // deno-lint-ignore no-explicit-any
  (Module as any).runMain();
  // deno-lint-ignore no-explicit-any
  assertEquals((globalThis as any).calledViaRunMain, true);
});

Deno.test("[node/module _nodeModulePaths] prevents duplicate /node_modules/node_modules suffix", () => {
  // deno-lint-ignore no-explicit-any
  const actual: string[] = (Module as any)._nodeModulePaths(
    path.join(process.cwd(), "testdata", "node_modules", "foo"),
  );

  assert(
    !actual.some((dir) => /node_modules[/\\]node_modules/g.test(dir)),
    "Duplicate 'node_modules/node_modules' suffix found",
  );
});

Deno.test("[node/module _nodeModulePaths] prevents duplicate root /node_modules", () => {
  // deno-lint-ignore no-explicit-any
  const actual: string[] = (Module as any)._nodeModulePaths(
    path.join(process.cwd(), "testdata", "node_modules", "foo"),
  );

  assert(
    new Set(actual).size === actual.length,
    "Duplicate path entries found",
  );
  const root = path.parse(actual[0]).root;
  assert(
    actual.includes(path.join(root, "node_modules")),
    "Missing root 'node_modules' directory",
  );
});

Deno.test("Built-in Node modules have `node:` prefix", () => {
  let thrown = false;
  try {
    // @ts-ignore We want to explicitly test wrong call signature
    createRequire();
  } catch (e) {
    thrown = true;
    const stackLines = e.stack.split("\n");
    // Assert that built-in node modules have `node:<mod_name>` specifiers.
    assert(stackLines.some((line: string) => line.includes("(node:module:")));
  }

  assert(thrown);
});
