// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const array = loadTestLibrary();

Deno.test("napi array new", function () {
  const e = [0, "Hello", {}];
  const r = array.test_array_new(e);
  assertEquals(typeof r, "object");
  assertEquals(r.length, 3);
  assertEquals(e, r);
});

Deno.test("napi array new with length", function () {
  const r = array.test_array_new_with_length(100);
  assertEquals(typeof r, "object");
  assertEquals(r.length, 100);
});
