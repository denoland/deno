// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";
import { Buffer } from "node:buffer";

const lib = loadTestLibrary();

Deno.test("napi create_buffer", function () {
  const buf = lib.test_create_buffer();
  assertEquals(buf instanceof Buffer, true);
  assertEquals(buf.length, 10);
  for (let i = 0; i < 10; i++) {
    assertEquals(buf[i], i);
  }
});

Deno.test("napi create_buffer_copy", function () {
  const buf = lib.test_create_buffer_copy();
  assertEquals(buf instanceof Buffer, true);
  assertEquals(buf.length, 5);
  assertEquals(buf[0], 10);
  assertEquals(buf[1], 20);
  assertEquals(buf[2], 30);
  assertEquals(buf[3], 40);
  assertEquals(buf[4], 50);
});

Deno.test("napi get_buffer_info", function () {
  const len = lib.test_get_buffer_info();
  assertEquals(len, 3);
});

Deno.test("napi is_buffer", function () {
  const result = lib.test_is_buffer_check();
  assertEquals(result, true);
});
