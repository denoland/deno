// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { Module } from "node:module";
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";
import process from "node:process";

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
