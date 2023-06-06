// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as path from "../../../../test_util/std/path/mod.ts";
import { Buffer } from "node:buffer";
import {
  assert,
  assertEquals,
} from "../../../../test_util/std/testing/asserts.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = path.resolve(moduleDir, "testdata", "hello.txt");
const fs = await import("node:fs/promises");
const decoder = new TextDecoder();

Deno.test("readFileSuccess", async function () {
  const fileHandle = await fs.open(testData);
  const data = await fileHandle.readFile();

  assert(data instanceof Uint8Array);
  assertEquals(decoder.decode(data as Uint8Array), "hello world");

  await fileHandle.close();
});

Deno.test("write", async function () {
  const tempFile: string = await Deno.makeTempFile();
  const fileHandle = await fs.open(tempFile, "a+");

  const buffer = Buffer.from("hello world");
  const res = await fileHandle.write(buffer, 0, 5, 0);

  const data = Deno.readFileSync(tempFile);
  await Deno.remove(tempFile);
  await fileHandle.close();

  assertEquals(res.bytesWritten, 5);
  assertEquals(decoder.decode(data), "hello");
});

Deno.test("write with opt", async function () {
  const tempFile: string = await Deno.makeTempFile();
  const fileHandle = await fs.open(tempFile);

  const buffer = Buffer.from("hello world");
  const opt = {
    offset: 0,
    length: 5,
    potition: 0,
  };
  const res = await fileHandle.write(buffer, opt);

  const data = Deno.readFileSync(tempFile);
  await Deno.remove(tempFile);
  await fileHandle.close();

  assertEquals(res.bytesWritten, 5);
  assertEquals(decoder.decode(data), "hello");
});
