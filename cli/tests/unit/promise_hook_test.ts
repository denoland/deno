// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assertEquals, unitTest } from "./test_util.ts";

const twoAwaitResolveThenRejectWithCatch = [
  0,
  0,
  1,
  0,
  0,
  0,
  0,
  0,
  0,
  2,
  0,
  1,
  0,
  1,
  3,
  2,
  0,
  1,
  0,
  1,
  1,
  3,
  2,
  1,
  3,
  2,
];
unitTest(function promiseHookConstants() {
  assertEquals(Deno.core.setPromiseHook.INIT, 0);
  assertEquals(Deno.core.setPromiseHook.RESOLVE, 1);
  assertEquals(Deno.core.setPromiseHook.BEFORE, 2);
  assertEquals(Deno.core.setPromiseHook.AFTER, 3);
});
unitTest(async function promiseHookBasic() {
  const hookResults: number[] = [];
  Deno.core.setPromiseHook((type: number) => {
    hookResults.push(type);
  });

  async function asyncFn() {
    await Promise.resolve(15);
    await Promise.resolve(20);
    Promise.reject(new Error()).catch(() => {});
  }
  await asyncFn();
  assertEquals(hookResults, twoAwaitResolveThenRejectWithCatch);
});

unitTest(async function promiseHookMultipleConsumers() {
  const hookResultsFirstConsumer: number[] = [];
  const hookResultsSecondConsumer: number[] = [];
  Deno.core.setPromiseHook((type: number) => {
    hookResultsFirstConsumer.push(type);
  });
  Deno.core.setPromiseHook((type: number) => {
    hookResultsSecondConsumer.push(type);
  });

  async function asyncFn() {
    await Promise.resolve(15);
    await Promise.resolve(20);
    Promise.reject(new Error()).catch(() => {});
  }
  await asyncFn();
  assertEquals(
    hookResultsFirstConsumer,
    hookResultsSecondConsumer,
  );
});
