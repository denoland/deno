// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { readLong } from "./read_long.ts";
import { sliceLongToBytes } from "./slice_long_to_bytes.ts";
import { BufReader } from "./buf_reader.ts";
import { BinaryReader } from "./_test_common.ts";

Deno.test("testSliceLongToBytes", function () {
  const arr = sliceLongToBytes(0x1234567890abcdef);
  const actual = readLong(new BufReader(new BinaryReader(new Uint8Array(arr))));
  const expected = readLong(
    new BufReader(
      new BinaryReader(
        new Uint8Array([0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef]),
      ),
    ),
  );
  assertEquals(actual, expected);
});

Deno.test("testSliceLongToBytes2", function () {
  const arr = sliceLongToBytes(0x12345678);
  assertEquals(arr, [0, 0, 0, 0, 0x12, 0x34, 0x56, 0x78]);
});
