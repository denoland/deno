// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi callback scope open and close", function () {
  assertEquals(lib.test_callback_scope(), true);
});

Deno.test("napi make_callback with async context", function () {
  const result = lib.test_make_callback_with_async_context(() => 42);
  assertEquals(result, 42);
});
