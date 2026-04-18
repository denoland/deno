// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, assertThrows, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi callback scope open and close", function () {
  assertEquals(lib.test_callback_scope(), true);
});

Deno.test("napi make_callback with async context", function () {
  const result = lib.test_make_callback_with_async_context(() => 42);
  assertEquals(result, 42);
});

Deno.test("napi async context lifecycle", function () {
  // Tests napi_async_init with and without resource, and napi_async_destroy
  assertEquals(lib.test_async_context_lifecycle(), true);
});

Deno.test("napi make_callback with real async context", function () {
  const result = lib.test_make_callback_with_real_context(() => 99);
  assertEquals(result, 99);
});

// Ported from Node.js test_callback_scope: RunInCallbackScope
Deno.test("napi run in callback scope", function () {
  const result = lib.test_run_in_callback_scope({}, "test-resource", () => 42);
  assertEquals(result, 42);
});

// Ported from Node.js test_callback_scope: RunInCallbackScope with exception
Deno.test("napi run in callback scope with exception", function () {
  assertThrows(
    () => {
      lib.test_run_in_callback_scope({}, "test-resource", () => {
        throw new Error("callback error");
      });
    },
    Error,
    "callback error",
  );
});
