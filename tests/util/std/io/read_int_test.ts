// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { readInt } from "./read_int.ts";
import { BufReader } from "./buf_reader.ts";
import { BinaryReader } from "./_test_common.ts";

Deno.test("testReadInt", async function () {
  const r = new BinaryReader(new Uint8Array([0x12, 0x34, 0x56, 0x78]));
  const int = await readInt(new BufReader(r));
  assertEquals(int, 0x12345678);
});
