// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function getRandomValuesInt8Array(): void {
  const arr = new Int8Array(32);
  crypto.getRandomValues(arr);
  assert.notEquals(arr, new Int8Array(32));
});

test(function getRandomValuesUint8Array(): void {
  const arr = new Uint8Array(32);
  crypto.getRandomValues(arr);
  assert.notEquals(arr, new Uint8Array(32));
});

test(function getRandomValuesUint8ClampedArray(): void {
  const arr = new Uint8ClampedArray(32);
  crypto.getRandomValues(arr);
  assert.notEquals(arr, new Uint8ClampedArray(32));
});

test(function getRandomValuesInt16Array(): void {
  const arr = new Int16Array(4);
  crypto.getRandomValues(arr);
  assert.notEquals(arr, new Int16Array(4));
});

test(function getRandomValuesUint16Array(): void {
  const arr = new Uint16Array(4);
  crypto.getRandomValues(arr);
  assert.notEquals(arr, new Uint16Array(4));
});

test(function getRandomValuesInt32Array(): void {
  const arr = new Int32Array(8);
  crypto.getRandomValues(arr);
  assert.notEquals(arr, new Int32Array(8));
});

test(function getRandomValuesUint32Array(): void {
  const arr = new Uint32Array(8);
  crypto.getRandomValues(arr);
  assert.notEquals(arr, new Uint32Array(8));
});

test(function getRandomValuesReturnValue(): void {
  const arr = new Uint32Array(8);
  const rtn = crypto.getRandomValues(arr);
  assert.notEquals(arr, new Uint32Array(8));
  assert.strictEq(rtn, arr);
});
