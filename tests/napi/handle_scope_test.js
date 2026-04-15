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

// Ported from Node.js test_handle_scope: NewScopeWithException
// Verifies that closing a handle scope works while an exception is pending.
Deno.test("napi handle scope with pending exception", function () {
  try {
    lib.test_scope_with_exception(() => {
      throw new RangeError("test exception");
    });
  } catch (e) {
    // The exception from the callback is expected to propagate
    assertEquals(e instanceof RangeError, true);
  }
});

Deno.test("napi handle scope stress test (many handles)", function () {
  const result = lib.test_handle_scope_many_handles();
  assertEquals(result, true);
});
