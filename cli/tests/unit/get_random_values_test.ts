// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertNotEquals, assertStrictEquals } from "./test_util.ts";

Deno.test("getRandomValuesInt8Array", function (): void {
  const arr = new Int8Array(32);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Int8Array(32));
});

Deno.test("getRandomValuesUint8Array", function (): void {
  const arr = new Uint8Array(32);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Uint8Array(32));
});

Deno.test("getRandomValuesUint8ClampedArray", function (): void {
  const arr = new Uint8ClampedArray(32);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Uint8ClampedArray(32));
});

Deno.test("getRandomValuesInt16Array", function (): void {
  const arr = new Int16Array(4);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Int16Array(4));
});

Deno.test("getRandomValuesUint16Array", function (): void {
  const arr = new Uint16Array(4);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Uint16Array(4));
});

Deno.test("getRandomValuesInt32Array", function (): void {
  const arr = new Int32Array(8);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Int32Array(8));
});

Deno.test("getRandomValuesUint32Array", function (): void {
  const arr = new Uint32Array(8);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Uint32Array(8));
});

Deno.test("getRandomValuesReturnValue", function (): void {
  const arr = new Uint32Array(8);
  const rtn = crypto.getRandomValues(arr);
  assertNotEquals(arr, new Uint32Array(8));
  assertStrictEquals(rtn, arr);
});
