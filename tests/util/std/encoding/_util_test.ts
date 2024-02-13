// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrows } from "../assert/mod.ts";
import { validateBinaryLike } from "./_util.ts";

Deno.test("validateBinaryLike", () => {
  assertEquals(validateBinaryLike("hello"), new TextEncoder().encode("hello"));
  assertEquals(
    validateBinaryLike(new Uint8Array([1, 2, 3])),
    new Uint8Array([1, 2, 3]),
  );
  assertEquals(
    validateBinaryLike(new Uint8Array([1, 2, 3]).buffer),
    new Uint8Array([1, 2, 3]),
  );
});

Deno.test("validateBinaryLike with invalid inputs", () => {
  assertThrows(
    () => {
      validateBinaryLike(1);
    },
    TypeError,
    "The input must be a Uint8Array, a string, or an ArrayBuffer. Received a value of the type number.",
  );
  assertThrows(
    () => {
      validateBinaryLike(undefined);
    },
    TypeError,
    "The input must be a Uint8Array, a string, or an ArrayBuffer. Received a value of the type undefined.",
  );
  assertThrows(
    () => {
      validateBinaryLike(null);
    },
    TypeError,
    "The input must be a Uint8Array, a string, or an ArrayBuffer. Received a value of the type null.",
  );
  assertThrows(
    () => {
      validateBinaryLike({});
    },
    TypeError,
    "The input must be a Uint8Array, a string, or an ArrayBuffer. Received a value of the type Object.",
  );
  assertThrows(
    () => {
      validateBinaryLike(new class MyClass {}());
    },
    TypeError,
    "The input must be a Uint8Array, a string, or an ArrayBuffer. Received a value of the type MyClass.",
  );
});
