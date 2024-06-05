// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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
