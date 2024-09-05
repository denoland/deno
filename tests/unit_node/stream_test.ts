// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "@std/assert";
import { fromFileUrl, relative } from "@std/path";
import { pipeline } from "node:stream/promises";
// @ts-expect-error: @types/node is outdated
import { getDefaultHighWaterMark } from "node:stream";
import { createReadStream, createWriteStream } from "node:fs";

Deno.test("stream/promises pipeline", async () => {
  const filePath = relative(
    Deno.cwd(),
    fromFileUrl(new URL("./testdata/lorem_ipsum.txt", import.meta.url)),
  );
  const input = createReadStream(filePath);
  const output = createWriteStream("lorem_ipsum.txt.copy");

  await pipeline(input, output);

  const content = Deno.readTextFileSync("lorem_ipsum.txt.copy");
  assert(content.startsWith("Lorem ipsum dolor sit amet"));
  try {
    Deno.removeSync("lorem_ipsum.txt.copy");
  } catch {
    // pass
  }
});

Deno.test("stream getDefaultHighWaterMark", () => {
  assertEquals(getDefaultHighWaterMark(false), 16 * 1024);
  assertEquals(getDefaultHighWaterMark(true), 16);
});
