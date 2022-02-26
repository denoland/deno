// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertNotEquals, assertStrictEquals } from "./test_util.ts";

Deno.test(function getRandomValuesInt8Array() {
  const arr = new Int8Array(32);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Int8Array(32));
});

Deno.test(function getRandomValuesUint8Array() {
  const arr = new Uint8Array(32);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Uint8Array(32));
});

Deno.test(function getRandomValuesUint8ClampedArray() {
  const arr = new Uint8ClampedArray(32);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Uint8ClampedArray(32));
});

Deno.test(function getRandomValuesInt16Array() {
  const arr = new Int16Array(4);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Int16Array(4));
});

Deno.test(function getRandomValuesUint16Array() {
  const arr = new Uint16Array(4);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Uint16Array(4));
});

Deno.test(function getRandomValuesInt32Array() {
  const arr = new Int32Array(8);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Int32Array(8));
});

Deno.test(function getRandomValuesUint32Array() {
  const arr = new Uint32Array(8);
  crypto.getRandomValues(arr);
  assertNotEquals(arr, new Uint32Array(8));
});

Deno.test(function getRandomValuesReturnValue() {
  const arr = new Uint32Array(8);
  const rtn = crypto.getRandomValues(arr);
  assertNotEquals(arr, new Uint32Array(8));
  assertStrictEquals(rtn, arr);
});
