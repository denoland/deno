// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { readLong } from "./read_long.ts";
import { BufReader } from "./buf_reader.ts";
import { BinaryReader } from "./_test_common.ts";

Deno.test("testReadLong", async function () {
  const r = new BinaryReader(
    new Uint8Array([0x00, 0x00, 0x00, 0x78, 0x12, 0x34, 0x56, 0x78]),
  );
  const long = await readLong(new BufReader(r));
  assertEquals(long, 0x7812345678);
});

Deno.test("testReadLong2", async function () {
  const r = new BinaryReader(
    new Uint8Array([0, 0, 0, 0, 0x12, 0x34, 0x56, 0x78]),
  );
  const long = await readLong(new BufReader(r));
  assertEquals(long, 0x12345678);
});
