// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { test } = Deno;
import { assert, assertEquals, assertStrictEq } from "../testing/asserts.ts";
import { collectUint8Arrays, deferred, MuxAsyncIterator } from "./async.ts";

test(function asyncDeferred(): Promise<void> {
  const d = deferred<number>();
  d.resolve(12);
  return Promise.resolve();
});

// eslint-disable-next-line require-await
async function* gen123(): AsyncIterableIterator<number> {
  yield 1;
  yield 2;
  yield 3;
}

// eslint-disable-next-line require-await
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

test(async function collectUint8Arrays0(): Promise<void> {
  async function* gen(): AsyncIterableIterator<Uint8Array> {}
  const result = await collectUint8Arrays(gen());
  assert(result instanceof Uint8Array);
  assertEquals(result.length, 0);
});

test(async function collectUint8Arrays0(): Promise<void> {
  async function* gen(): AsyncIterableIterator<Uint8Array> {}
  const result = await collectUint8Arrays(gen());
  assert(result instanceof Uint8Array);
  assertStrictEq(result.length, 0);
});

test(async function collectUint8Arrays1(): Promise<void> {
  const buf = new Uint8Array([1, 2, 3]);
  // eslint-disable-next-line require-await
  async function* gen(): AsyncIterableIterator<Uint8Array> {
    yield buf;
  }
  const result = await collectUint8Arrays(gen());
  assertStrictEq(result, buf);
  assertStrictEq(result.length, 3);
});

test(async function collectUint8Arrays4(): Promise<void> {
  // eslint-disable-next-line require-await
  async function* gen(): AsyncIterableIterator<Uint8Array> {
    yield new Uint8Array([1, 2, 3]);
    yield new Uint8Array([]);
    yield new Uint8Array([4, 5]);
    yield new Uint8Array([6]);
  }
  const result = await collectUint8Arrays(gen());
  assert(result instanceof Uint8Array);
  assertStrictEq(result.length, 6);
  for (let i = 0; i < 6; i++) {
    assertStrictEq(result[i], i + 1);
  }
});
