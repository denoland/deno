// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import * as colors from "../../../test_util/std/fmt/colors.ts";
export { colors };
import { resolve } from "../../../test_util/std/path/mod.ts";
export {
  assert,
  assertEquals,
  assertFalse,
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
export { readLines } from "../../../test_util/std/io/buffer.ts";
export { parse as parseArgs } from "../../../test_util/std/flags/mod.ts";

export function pathToAbsoluteFileUrl(path: string): URL {
  path = resolve(path);

  return new URL(`file://${Deno.build.os === "windows" ? "/" : ""}${path}`);
}

const decoder = new TextDecoder();

export async function execCode(code: string): Promise<[number, string]> {
  const output = await Deno.spawn(Deno.execPath(), {
    args: [
      "eval",
      "--unstable",
      "--no-check",
      code,
    ],
  });
  return [output.code, decoder.decode(output.stdout)];
}
