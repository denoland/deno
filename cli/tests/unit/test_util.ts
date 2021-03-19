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
  assertStrictEquals,
  assertStringIncludes,
  assertThrows,
  assertThrowsAsync,
  fail,
  unimplemented,
  unreachable,
} from "../../../test_util/std/testing/asserts.ts";
export { deferred } from "../../../test_util/std/async/deferred.ts";
export { readLines } from "../../../test_util/std/io/bufio.ts";
export { parse as parseArgs } from "../../../test_util/std/flags/mod.ts";

export function pathToAbsoluteFileUrl(path: string): URL {
  return new URL(
    `file://${Deno.build.os === "windows" ? "/" : ""}${resolve(path)}`,
  );
}
