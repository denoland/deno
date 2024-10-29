// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import * as path from "@std/path";
import { Buffer } from "node:buffer";
import * as fs from "node:fs/promises";
import { assert, assertEquals } from "@std/assert";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = path.resolve(moduleDir, "testdata", "hello.txt");
const decoder = new TextDecoder();

Deno.test("readFileSuccess", async function () {
  const fileHandle = await fs.open(testData);
  const data = await fileHandle.readFile();

  assert(data instanceof Uint8Array);
  assertEquals(decoder.decode(data as Uint8Array), "hello world");

  await fileHandle.close();
});

Deno.test("read", async function () {
  const fileHandle = await fs.open(testData);
  const byteLength = "hello world".length;

  const buf = new Buffer(byteLength);
  await fileHandle.read(buf, 0, byteLength, 0);

  assertEquals(decoder.decode(buf as Uint8Array), "hello world");

  await fileHandle.close();
});

Deno.test("read specify opt", async function () {
  const fileHandle = await fs.open(testData);
  const byteLength = "hello world".length;

  const opt = {
    buffer: new Buffer(byteLength),
    offset: 6,
    length: 5,
    position: 6,
  };
  let res = await fileHandle.read(opt);

  assertEquals(res.bytesRead, 5);
  assertEquals(
    new TextDecoder().decode(res.buffer.subarray(6) as Uint8Array),
    "world",
  );

  const opt2 = {
    buffer: new Buffer(byteLength),
    length: 5,
    position: 0,
  };
  res = await fileHandle.read(opt2);

  assertEquals(res.bytesRead, 5);
  assertEquals(
    decoder.decode(res.buffer.subarray(0, 5) as Uint8Array),
    "hello",
  );

  await fileHandle.close();
});

Deno.test("[node/fs filehandle.write] Write from Buffer", async function () {
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

Deno.test("[node/fs filehandle.write] Write from string", async function () {
  const tempFile: string = await Deno.makeTempFile();
  const fileHandle = await fs.open(tempFile, "a+");

  const str = "hello world";
  const res = await fileHandle.write(str);

  const data = Deno.readFileSync(tempFile);
  await Deno.remove(tempFile);
  await fileHandle.close();

  assertEquals(res.bytesWritten, 11);
  assertEquals(decoder.decode(data), "hello world");
});
