// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

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

export function pathToAbsoluteFileUrl(path: string): URL {
  path = resolve(path);

  return new URL(`file://${Deno.build.os === "windows" ? "/" : ""}${path}`);
}
