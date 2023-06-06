// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as path from "../../../../test_util/std/path/mod.ts";
import {
  assert,
  assertEquals,
} from "../../../../test_util/std/testing/asserts.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = path.resolve(moduleDir, "testdata", "hello.txt");

Deno.test("readFileSuccess", async function () {
  const fs = await import("node:fs/promises");
  const fileHandle = await fs.open(testData);
  const data = await fileHandle.readFile();

  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");

  await fileHandle.close();
});

Deno.test("write", async function () {
  const tempFile: string = await Deno.makeTempFile();
  const fileHandle = await fs.open(tempFile);

  const buffer = Buffer.from("hello world");
  const bytesWrite = await filehandle.write(buffer, 0, 5, 0);
  fileHandle.close();

  const data = await fileHandle.readFile();
  await Deno.remove(tempFile);

  assertEquals(bytesWrite, 5);
  assertEquals(decoder.decode(data), "hello");
});

Deno.test("write with opt", async function () {
  const tempFile: string = await Deno.makeTempFile();
  const fileHandle = await fs.open(tempFile);

  const buffer = Buffer.from("hello world");
  const opt = {
    buffer,
    offset: 0,
    length: 5,
  };

  fileHandle.close();

  const data = await fileHandle.readFile();
  await Deno.remove(tempFile);

  assertEquals(bytesWrite, 5);
  assertEquals(decoder.decode(data), "hello");
});
