// Copyright 2018-2025 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertNotEquals,
  assertStringIncludes,
} from "./test_util.ts";

Deno.test(function customInspectFunction() {
  const exception = new DOMException("test");
  assertEquals(Deno.inspect(exception), exception.stack);
  assertStringIncludes(Deno.inspect(DOMException.prototype), "DOMException");
});

Deno.test(function nameToCodeMappingPrototypeAccess() {
  const newCode = 100;
  const objectPrototype = Object.prototype as unknown as {
    pollution: number;
  };
  objectPrototype.pollution = newCode;
  assertNotEquals(newCode, new DOMException("test", "pollution").code);
  Reflect.deleteProperty(objectPrototype, "pollution");
});

Deno.test(function hasStackAccessor() {
  const e2 = new DOMException("asdf");
  const desc = Object.getOwnPropertyDescriptor(e2, "stack");
  assert(desc);
  assert(typeof desc.get === "function");
  assert(typeof desc.set === "function");
});

Deno.test(function quotaExceededErrorIsSubclass() {
  const error = new QuotaExceededError("test message");
  assert(error instanceof QuotaExceededError);
  assert(error instanceof DOMException);
  assert(error instanceof Error);
});

Deno.test(function quotaExceededErrorCodeIsZero() {
  // QuotaExceededError is now a subclass, not a DOMException name
  // So creating a DOMException with name "QuotaExceededError" should have code 0
  const error = new DOMException("test", "QuotaExceededError");
  assertEquals(error.code, 0);
});

Deno.test(function quotaExceededErrorHasCorrectName() {
  const error = new QuotaExceededError("test message");
  assertEquals(error.name, "QuotaExceededError");
});

Deno.test(function quotaExceededErrorHasQuotaAndRequestedProperties() {
  const error = new QuotaExceededError("test message");
  assertEquals(error.quota, null);
  assertEquals(error.requested, null);
});

Deno.test(function quotaExceededErrorWithOptions() {
  const error = new QuotaExceededError("test message", {
    quota: 1000,
    requested: 1500,
  });
  assertEquals(error.quota, 1000);
  assertEquals(error.requested, 1500);
  assertEquals(error.message, "test message");
  assertEquals(error.name, "QuotaExceededError");
});
