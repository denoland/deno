// Copyright 2018-2026 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertNotEquals,
  assertStringIncludes,
  assertThrows,
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

// QuotaExceededError still maps to code 22 in the DOMException names table
Deno.test(function domExceptionQuotaExceededErrorCode() {
  const e = new DOMException("test", "QuotaExceededError");
  assertEquals(e.code, 22);
  assertEquals(e.name, "QuotaExceededError");
});

// QuotaExceededError as a DOMException derived interface
Deno.test(function quotaExceededErrorBasic() {
  const e = new QuotaExceededError("quota exceeded");
  assert(e instanceof QuotaExceededError);
  assert(e instanceof DOMException);
  assert(e instanceof Error);
  assertEquals(e.name, "QuotaExceededError");
  assertEquals(e.message, "quota exceeded");
  assertEquals(e.code, 22);
  assertEquals(e.quota, null);
  assertEquals(e.requested, null);
});

Deno.test(function quotaExceededErrorWithOptions() {
  const e = new QuotaExceededError("too much", { quota: 100, requested: 200 });
  assertEquals(e.name, "QuotaExceededError");
  assertEquals(e.message, "too much");
  assertEquals(e.code, 22);
  assertEquals(e.quota, 100);
  assertEquals(e.requested, 200);
});

Deno.test(function quotaExceededErrorDefaultMessage() {
  const e = new QuotaExceededError();
  assertEquals(e.message, "");
  assertEquals(e.name, "QuotaExceededError");
  assertEquals(e.code, 22);
  assertEquals(e.quota, null);
  assertEquals(e.requested, null);
});

Deno.test(function quotaExceededErrorPartialOptions() {
  const e1 = new QuotaExceededError("msg", { quota: 50 });
  assertEquals(e1.quota, 50);
  assertEquals(e1.requested, null);

  const e2 = new QuotaExceededError("msg", { requested: 75 });
  assertEquals(e2.quota, null);
  assertEquals(e2.requested, 75);
});

Deno.test(function quotaExceededErrorNegativeQuotaThrows() {
  assertThrows(
    () => new QuotaExceededError("msg", { quota: -1 }),
    RangeError,
  );
});

Deno.test(function quotaExceededErrorNegativeRequestedThrows() {
  assertThrows(
    () => new QuotaExceededError("msg", { requested: -1 }),
    RangeError,
  );
});

Deno.test(function quotaExceededErrorLegacyCodeConstant() {
  assertEquals(DOMException.QUOTA_EXCEEDED_ERR, 22);
});

Deno.test(function quotaExceededErrorConstructorName() {
  const e = new QuotaExceededError("test");
  assertEquals(e.constructor, QuotaExceededError);
  assertEquals(e.constructor.name, "QuotaExceededError");
});

Deno.test(function quotaExceededErrorHasStack() {
  const e = new QuotaExceededError("test");
  assert(typeof e.stack === "string");
  assertStringIncludes(e.stack, "QuotaExceededError");
});
