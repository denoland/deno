// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

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
