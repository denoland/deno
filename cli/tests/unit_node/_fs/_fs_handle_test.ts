// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as path from "../../../../test_util/std/path/mod.ts";
import {
  assert,
  assertEquals,
} from "../../../../test_util/std/testing/asserts.ts";
import { Buffer } from "node:buffer";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = path.resolve(moduleDir, "testdata", "hello.txt");

Deno.test("readFileSuccess", async function () {
  const fs = await import("node:fs/promises");
  const fileHandle = await fs.open(testData);
  const data = await fileHandle.readFile();

  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");

  Deno.close(fileHandle.fd);
});

Deno.test("readSuccess", async function () {
  const fs = await import("node:fs/promises");
  const fileHandle = await fs.open(testData);

  const buf = new Buffer(16);
  await fileHandle.read(buf);

  assert(buf instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(buf as Uint8Array), "hello world");
});
