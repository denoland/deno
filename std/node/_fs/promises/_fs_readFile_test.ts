// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { readFile } from "./_fs_readFile.ts";
import { resolvePath } from "../../../fs/mod.ts";
import * as path from "../../../path/mod.ts";
import { assert, assertEquals } from "../../../testing/asserts.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = resolvePath(moduleDir, "..", "testdata", "hello.txt");

Deno.test("readFileSuccess", async function () {
  const data: Uint8Array = await readFile(testData);

  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data), "hello world");
});

Deno.test("readFileBinarySuccess", async function () {
  const data: Uint8Array = await readFile(testData, "binary");

  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data), "hello world");
});

Deno.test("readFileBinaryObjectSuccess", async function () {
  const data: Uint8Array = await readFile(testData, { encoding: "binary" });

  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data), "hello world");
});

Deno.test("readFileStringObjectSuccess", async function () {
  const data: string = await readFile(testData, { encoding: "utf8" });

  assertEquals(typeof data, "string");
  assertEquals(data, "hello world");
});

Deno.test("readFileEncodeHexSuccess", async function () {
  const data: string = await readFile(testData, { encoding: "hex" });
  assertEquals(typeof data, "string");
  assertEquals(data as string, "68656c6c6f20776f726c64");
});

Deno.test("readFileEncodeBase64Success", async function () {
  const data: string = await readFile(testData, { encoding: "base64" });
  assertEquals(typeof data, "string");
  assertEquals(data as string, "aGVsbG8gd29ybGQ=");
});

Deno.test("readFileStringSuccess", async function () {
  const data: string = await readFile(testData, "utf8");

  assertEquals(typeof data, "string");
  assertEquals(data, "hello world");
});

Deno.test("readFileError", async function () {
  try {
    await readFile("invalid-file", "utf8");
  } catch (e) {
    assert(e instanceof Deno.errors.NotFound);
  }
});
