// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { LimitedReader } from "./limited_reader.ts";
import { StringWriter } from "./string_writer.ts";
import { copy } from "../streams/copy.ts";
import { readAll } from "../streams/read_all.ts";
import { StringReader } from "./string_reader.ts";

Deno.test("ioLimitedReader", async function () {
  const decoder = new TextDecoder();
  let sr = new StringReader("abc");
  let r = new LimitedReader(sr, 2);
  let buffer = await readAll(r);
  assertEquals(decoder.decode(buffer), "ab");
  assertEquals(decoder.decode(await readAll(sr)), "c");
  sr = new StringReader("abc");
  r = new LimitedReader(sr, 3);
  buffer = await readAll(r);
  assertEquals(decoder.decode(buffer), "abc");
  assertEquals((await readAll(r)).length, 0);
  sr = new StringReader("abc");
  r = new LimitedReader(sr, 4);
  buffer = await readAll(r);
  assertEquals(decoder.decode(buffer), "abc");
  assertEquals((await readAll(r)).length, 0);
});

Deno.test("ioLimitedReader", async function () {
  const rb = new StringReader("abc");
  const wb = new StringWriter();
  await copy(new LimitedReader(rb, -1), wb);
  assertEquals(wb.toString(), "");
});
