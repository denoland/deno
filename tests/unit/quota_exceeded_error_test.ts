// Copyright 2018-2025 the Deno authors. MIT license.

import { assertEquals, assert } from "@std/assert";

Deno.test("QuotaExceededError is a subclass of DOMException", () => {
  const err = new QuotaExceededError();
  assert(err instanceof DOMException);
  assert(err instanceof QuotaExceededError);
});

Deno.test("QuotaExceededError has correct name and code", () => {
  const err = new QuotaExceededError();
  assertEquals(err.name, "QuotaExceededError");
  assertEquals(err.code, 22);
});

Deno.test("QuotaExceededError constructor", () => {
  const err = new QuotaExceededError("The quota has been exceeded.");
  assertEquals(err.message, "The quota has been exceeded.");
  assertEquals(err.name, "QuotaExceededError");
  assertEquals(err.code, 22);
});

Deno.test("new DOMException('...', 'QuotaExceededError')", () => {
  const err = new DOMException("The quota has been exceeded.", "QuotaExceededError");
  assert(err instanceof DOMException);
  assertEquals(err.name, "QuotaExceededError");
  assertEquals(err.message, "The quota has been exceeded.");
  // The code will be 0 when using the DOMException constructor with a name
  // that is not in the nameToCodeMapping. This is expected.
  assertEquals(err.code, 0);
});
