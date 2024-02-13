// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { StringReader } from "./string_reader.ts";

Deno.test("ioStringReader", async function () {
  const r = new StringReader("abcdef");
  const res0 = await r.read(new Uint8Array(6));
  assertEquals(res0, 6);
  const res1 = await r.read(new Uint8Array(6));
  assertEquals(res1, null);
});

Deno.test("ioStringReader", async function () {
  const decoder = new TextDecoder();
  const r = new StringReader("abcdef");
  const buf = new Uint8Array(3);
  const res1 = await r.read(buf);
  assertEquals(res1, 3);
  assertEquals(decoder.decode(buf), "abc");
  const res2 = await r.read(buf);
  assertEquals(res2, 3);
  assertEquals(decoder.decode(buf), "def");
  const res3 = await r.read(buf);
  assertEquals(res3, null);
  assertEquals(decoder.decode(buf), "def");
});

Deno.test("testStringReaderEof", async function () {
  const r = new StringReader("abc");
  assertEquals(await r.read(new Uint8Array()), 0);
  assertEquals(await r.read(new Uint8Array(4)), 3);
  assertEquals(await r.read(new Uint8Array(1)), null);
});
