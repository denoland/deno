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

export function execCode(code: string): Promise<readonly [number, string]> {
  return execCode2(code).finished();
}

export function execCode2(code: string) {
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "eval",
      "--unstable",
      "--no-check",
      code,
    ],
    stdout: "piped",
    stderr: "inherit",
  });

  const child = command.spawn();
  const stdout = child.stdout.pipeThrough(new TextDecoderStream()).getReader();
  let output = "";

  return {
    async waitStdoutText(text: string) {
      while (true) {
        const readData = await stdout.read();
        if (readData.value) {
          output += readData.value;
          if (output.includes(text)) {
            return;
          }
        }
        if (readData.done) {
          throw new Error(`Did not find text '${text}' in stdout.`);
        }
      }
    },
    async finished() {
      while (true) {
        const readData = await stdout.read();
        if (readData.value) {
          output += readData.value;
        }
        if (readData.done) {
          break;
        }
      }
      const status = await child.status;
      return [status.code, output] as const;
    },
  };
}
