// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import * as path from "@std/path/mod.ts";
import { Buffer } from "node:buffer";
import * as fs from "node:fs/promises";
import { assert, assertEquals } from "@std/assert/mod.ts";

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
    buffer: Buffer.alloc(byteLength),
    offset: 6,
    length: 5,
  };
  let res = await fileHandle.read(opt);

  assertEquals(res.bytesRead, 5);
  // deno-fmt-ignore
  assertEquals(res.buffer as Uint8Array, Buffer.from([
    0, 0, 0, 0, 0, 0,
    // hello
    0x68, 0x65, 0x6c, 0x6c, 0x6f,
  ]))

  const opt2 = {
    buffer: Buffer.alloc(byteLength),
    length: 5,
    position: 0,
  };
  res = await fileHandle.read(opt2);

  assertEquals(res.bytesRead, 5);
  // deno-fmt-ignore
  assertEquals(res.buffer as Uint8Array, Buffer.from([
    // hello
    0x68, 0x65, 0x6c, 0x6c, 0x6f,
    0, 0, 0, 0, 0, 0,
  ]))

  const opt3 = {
    buffer: Buffer.alloc(byteLength),
    offset: 6,
    length: 5,
    position: 6,
  };
  res = await fileHandle.read(opt3);

  assertEquals(res.bytesRead, 5);
  // deno-fmt-ignore
  assertEquals(res.buffer as Uint8Array, Buffer.from([
    0, 0, 0, 0, 0, 0,
    // world
    0x77, 0x6f, 0x72, 0x6c, 0x64,
  ]))

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
