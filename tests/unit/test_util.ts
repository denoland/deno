// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import * as colors from "@std/fmt/colors.ts";
import { assert } from "@std/assert/mod.ts";
export { colors };
import { join, resolve } from "@std/path/mod.ts";
export {
  assert,
  assertEquals,
  assertFalse,
  AssertionError,
  assertIsError,
  assertMatch,
  assertNotEquals,
  assertNotStrictEquals,
  assertRejects,
  assertStrictEquals,
  assertStringIncludes,
  assertThrows,
  fail,
  unimplemented,
  unreachable,
} from "@std/assert/mod.ts";
export { delay } from "@std/async/delay.ts";
export { readLines } from "@std/io/read_lines.ts";
export { parse as parseArgs } from "@std/flags/mod.ts";

export const DENO_FUTURE = Deno.env.get("DENO_FUTURE") === "1";

export function pathToAbsoluteFileUrl(path: string): URL {
  path = resolve(path);

  return new URL(`file://${Deno.build.os === "windows" ? "/" : ""}${path}`);
}

export function execCode(code: string): Promise<readonly [number, string]> {
  return execCode2(code).finished();
}

export function execCode3(cmd: string, args: string[]) {
  const command = new Deno.Command(cmd, {
    args,
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

export function execCode2(code: string) {
  return execCode3(Deno.execPath(), ["eval", "--unstable", "--no-check", code]);
}

export function tmpUnixSocketPath(): string {
  const folder = Deno.makeTempDirSync();
  return join(folder, "socket");
}

export async function curlRequest(args: string[]) {
  const { success, stdout, stderr } = await new Deno.Command("curl", {
    args,
    stdout: "piped",
    stderr: "piped",
  }).output();
  const decoder = new TextDecoder();
  assert(
    success,
    `Failed to cURL ${args}: stdout\n\n${
      decoder.decode(stdout)
    }\n\nstderr:\n\n${decoder.decode(stderr)}`,
  );
  return decoder.decode(stdout);
}

export async function curlRequestWithStdErr(args: string[]) {
  const { success, stdout, stderr } = await new Deno.Command("curl", {
    args,
    stdout: "piped",
    stderr: "piped",
  }).output();
  const decoder = new TextDecoder();
  assert(
    success,
    `Failed to cURL ${args}: stdout\n\n${
      decoder.decode(stdout)
    }\n\nstderr:\n\n${decoder.decode(stderr)}`,
  );
  return [decoder.decode(stdout), decoder.decode(stderr)];
}
