// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as path from "../../../../test_util/std/path/mod.ts";
import {
  assert,
  assertEquals,
} from "../../../../test_util/std/testing/asserts.ts";
import { Buffer } from "node:buffer";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = path.resolve(moduleDir, "testdata", "hello.txt");
const fs = await import("node:fs/promises");

Deno.test("readFileSuccess", async function () {
  const fileHandle = await fs.open(testData);
  const data = await fileHandle.readFile();

  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");

  await fileHandle.close();
});

Deno.test("read", async function () {
  const fileHandle = await fs.open(testData);
  const byteLength = "hello world".length;

  const buf = new Buffer(byteLength);
  await fileHandle.read(buf, 0, byteLength, 0);

  assertEquals(new TextDecoder().decode(buf as Uint8Array), "hello world");

  await fileHandle.close();
});

Deno.test("read specify opt", async function () {
  const fileHandle = await fs.open(testData);
  const byteLength = "hello world".length;

  const opt = {
    buffer: new Buffer(byteLength),
    offset: 6,
    length: 5,
  };
  let res = await fileHandle.read(opt);

  assertEquals(res.bytesRead, byteLength);
  assertEquals(new TextDecoder().decode(res.buffer as Uint8Array), "world");

  const opt2 = {
    buffer: new Buffer(byteLength),
    length: 5,
    position: 0,
  };
  res = await fileHandle.read(opt2);

  assertEquals(res.bytesRead, byteLength);
  assertEquals(new TextDecoder().decode(res.buffer as Uint8Array), "hello");

  await fileHandle.close();
});
