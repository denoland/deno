// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, runIfMain } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { MuxAsyncIterator, deferred } from "./async.ts";

test(async function asyncDeferred(): Promise<void> {
  const d = deferred<number>();
  d.resolve(12);
});

async function* gen123(): AsyncIterableIterator<number> {
  yield 1;
  yield 2;
  yield 3;
}

async function* gen456(): AsyncIterableIterator<number> {
  yield 4;
  yield 5;
  yield 6;
}

test(async function asyncMuxAsyncIterator(): Promise<void> {
  const mux = new MuxAsyncIterator<number>();
  mux.add(gen123());
  mux.add(gen456());
  const results = new Set();
  for await (const value of mux) {
    results.add(value);
  }
  assertEquals(results.size, 6);
});

runIfMain(import.meta);
