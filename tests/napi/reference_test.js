// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi strong reference create/get/delete", function () {
  const result = lib.test_reference_strong();
  assertEquals(typeof result, "object");
  assertEquals(result.marker, 123);
});

Deno.test("napi reference ref/unref counting", function () {
  const result = lib.test_reference_ref_unref();
  assertEquals(result, true);
});

Deno.test("napi create_external / get_value_external", function () {
  const result = lib.test_create_external();
  assertEquals(result, 42);
});

Deno.test("napi external with reference", function () {
  const result = lib.test_create_external_reference();
  assertEquals(result, 99);
});
