// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { copyN } from "./copy_n.ts";
import { Buffer } from "./buffer.ts";
import { StringReader } from "./string_reader.ts";

Deno.test("testCopyN1", async function () {
  const w = new Buffer();
  const r = new StringReader("abcdefghij");
  const n = await copyN(r, w, 3);
  assertEquals(n, 3);
  assertEquals(new TextDecoder().decode(w.bytes()), "abc");
});

Deno.test("testCopyN2", async function () {
  const w = new Buffer();
  const r = new StringReader("abcdefghij");
  const n = await copyN(r, w, 11);
  assertEquals(n, 10);
  assertEquals(new TextDecoder().decode(w.bytes()), "abcdefghij");
});

Deno.test("copyNWriteAllData", async function () {
  const tmpDir = await Deno.makeTempDir();
  const filepath = `${tmpDir}/data`;
  const file = await Deno.open(filepath, { create: true, write: true });

  const size = 16 * 1024 + 1;
  const data = "a".repeat(32 * 1024);
  const r = new StringReader(data);
  const n = await copyN(r, file, size); // Over max file possible buffer
  file.close();
  await Deno.remove(filepath);

  assertEquals(n, size);
});
