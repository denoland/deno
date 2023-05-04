// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { Module } from "node:module";
import { assertStrictEquals } from "../../../test_util/std/testing/asserts.ts";

Deno.test("[node/module _preloadModules] has internal require hook", () => {
  // Check if it's there
  // deno-lint-ignore no-explicit-any
  (Module as any)._preloadModules([
    "./cli/tests/unit_node/testdata/add_global_property.js",
  ]);
  // deno-lint-ignore no-explicit-any
  assertStrictEquals((globalThis as any).foo, "Hello");
});
