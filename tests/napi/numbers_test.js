// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const numbers = loadTestLibrary();

Deno.test("napi int32", function () {
  assertEquals(numbers.test_int32(69), 69);
  assertEquals(numbers.test_int32(Number.MAX_SAFE_INTEGER), -1);
});

Deno.test("napi int64", function () {
  assertEquals(numbers.test_int64(69), 69);
  assertEquals(
    numbers.test_int64(Number.MAX_SAFE_INTEGER),
    Number.MAX_SAFE_INTEGER,
  );
});

Deno.test("napi double", function () {
  assertEquals(numbers.test_double(3.14), 3.14);
  assertEquals(numbers.test_double(-0.5), -0.5);
  assertEquals(numbers.test_double(0), 0);
});

Deno.test("napi uint32", function () {
  assertEquals(numbers.test_uint32(42), 42);
  assertEquals(numbers.test_uint32(0), 0);
  assertEquals(numbers.test_uint32(0xFFFFFFFF), 0xFFFFFFFF);
});
