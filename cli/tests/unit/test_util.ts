// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertEquals,
} from "../../../test_util/std/testing/asserts.ts";
import * as colors from "../../../test_util/std/fmt/colors.ts";
export { colors };
import { resolve } from "../../../test_util/std/path/mod.ts";
export {
  assert,
  assertEquals,
  assertMatch,
  assertNotEquals,
  assertRejects,
  assertStrictEquals,
  assertStringIncludes,
  assertThrows,
  fail,
  unimplemented,
  unreachable,
} from "../../../test_util/std/testing/asserts.ts";
export { deferred } from "../../../test_util/std/async/deferred.ts";
export type { Deferred } from "../../../test_util/std/async/deferred.ts";
export { delay } from "../../../test_util/std/async/delay.ts";
export { readLines } from "../../../test_util/std/io/bufio.ts";
export { parse as parseArgs } from "../../../test_util/std/flags/mod.ts";

interface UnitTestPermissions {
  env?: "inherit" | boolean | string[];
  hrtime?: "inherit" | boolean;
  net?: "inherit" | boolean | string[];
  ffi?: "inherit" | boolean;
  read?: "inherit" | boolean | Array<string | URL>;
  run?: "inherit" | boolean | Array<string | URL>;
  write?: "inherit" | boolean | Array<string | URL>;
}

interface UnitTestOptions {
  ignore?: boolean;
  only?: boolean;
  permissions?: UnitTestPermissions;
}

type TestFunction = () => void | Promise<void>;

export function unitTest(fn: TestFunction): void;
export function unitTest(options: UnitTestOptions, fn: TestFunction): void;
export function unitTest(
  optionsOrFn: UnitTestOptions | TestFunction,
  maybeFn?: TestFunction,
): void {
  assert(optionsOrFn, "At least one argument is required");

  let options: UnitTestOptions;
  let name: string;
  let fn: TestFunction;

  if (typeof optionsOrFn === "function") {
    options = {};
    fn = optionsOrFn;
    name = fn.name;
    assert(name, "Missing test function name");
  } else {
    options = optionsOrFn;
    assert(maybeFn, "Missing test function definition");
    assert(
      typeof maybeFn === "function",
      "Second argument should be test function definition",
    );
    fn = maybeFn;
    name = fn.name;
    assert(name, "Missing test function name");
  }

  const testDefinition: Deno.TestDefinition = {
    name,
    fn,
    ignore: !!options.ignore,
    only: !!options.only,
    permissions: Object.assign({
      read: false,
      write: false,
      net: false,
      env: false,
      run: false,
      ffi: false,
      hrtime: false,
    }, options.permissions),
  };

  Deno.test(testDefinition);
}

export function pathToAbsoluteFileUrl(path: string): URL {
  path = resolve(path);

  return new URL(`file://${Deno.build.os === "windows" ? "/" : ""}${path}`);
}
