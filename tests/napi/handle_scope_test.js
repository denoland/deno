// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi handle scope open/close", function () {
  const result = lib.test_open_close_scope();
  assertEquals(result, "ok");
});

Deno.test("napi escapable handle scope", function () {
  const result = lib.test_escapable_scope();
  assertEquals(result, "escaped");
});

Deno.test("napi escape handle twice returns error", function () {
  const result = lib.test_escape_twice();
  assertEquals(result, true);
});

Deno.test("napi nested handle scopes", function () {
  const result = lib.test_nested_scopes();
  assertEquals(result, true);
});
