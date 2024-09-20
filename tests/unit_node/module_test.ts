// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import {
  builtinModules,
  createRequire,
  findSourceMap,
  isBuiltin,
  Module,
  // @ts-ignore Our internal @types/node is at v18.16.19 which predates
  // this change. Updating it is difficult due to different types in Node
  // for `import.meta.filename` and `import.meta.dirname` that Deno
  // provides.
  register,
} from "node:module";
import { assert, assertEquals } from "@std/assert";
import process from "node:process";
import * as path from "node:path";

Deno.test("[node/module _preloadModules] has internal require hook", () => {
  // Check if it's there
  // deno-lint-ignore no-explicit-any
  (Module as any)._preloadModules([
    "./tests/unit_node/testdata/add_global_property.js",
  ]);
  // deno-lint-ignore no-explicit-any
  assertEquals((globalThis as any).foo, "Hello");
});

Deno.test("[node/module runMain] loads module using the current process.argv", () => {
  process.argv = [
    process.argv[0],
    "./tests/unit_node/testdata/add_global_property_run_main.js",
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
    const stackLines = (e as Error).stack!.split("\n");
    // Assert that built-in node modules have `node:<mod_name>` specifiers.
    assert(stackLines.some((line: string) => line.includes("(node:module:")));
  }

  assert(thrown);
});

Deno.test("[node/module isBuiltin] recognizes node builtins", () => {
  assert(isBuiltin("node:fs"));
  assert(isBuiltin("node:test"));
  assert(isBuiltin("fs"));
  assert(isBuiltin("buffer"));

  assert(!isBuiltin("internal/errors"));
  assert(!isBuiltin("test"));
  assert(!isBuiltin(""));
  // deno-lint-ignore no-explicit-any
  assert(!isBuiltin(undefined as any));
});

// https://github.com/denoland/deno/issues/22731
Deno.test("[node/module builtinModules] has 'module' in builtins", () => {
  assert(builtinModules.includes("module"));
});

// https://github.com/denoland/deno/issues/18666
Deno.test("[node/module findSourceMap] is a function", () => {
  assertEquals(findSourceMap("foo"), undefined);
});

// https://github.com/denoland/deno/issues/24902
Deno.test("[node/module register] is a function", () => {
  assertEquals(register("foo"), undefined);
});
