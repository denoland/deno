// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assertNotEquals } from "./test_util.ts";

test(async function csprngBytes(): Promise<void> {
  const arr = new Uint8Array(32);
  await Deno.getRandomValues(arr);
  assertNotEquals(arr, new Uint8Array(32));
});

test(async function csprngValues(): Promise<void> {
  const arr = new Int16Array(4);
  await Deno.getRandomValues(arr);
  assertNotEquals(arr, new Int16Array(4));
});

test(async function csprngBigints(): Promise<void> {
  const arr = new BigInt64Array(4);
  await Deno.getRandomValues(arr);
  assertNotEquals(arr, new BigInt64Array(4));
});

test(async function csprngGivenADataView(): Promise<void> {
  const arr = new Int32Array(4);
  const view = new DataView(arr.buffer);
  await Deno.getRandomValues(view);
  assertNotEquals(arr, new Int32Array(4));
});

test(function csprngValuesSync(): void {
  const arr = new Uint32Array(8);
  Deno.getRandomValuesSync(arr);
  assertNotEquals(arr, new Uint32Array(8));
});
