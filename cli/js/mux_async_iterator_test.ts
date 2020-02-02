// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assertEquals, assertThrowsAsync } from "./test_util.ts";
import { mux } from "./mux_async_iterator.ts";

function defer(n: number): Promise<void> {
  return new Promise((resolve, _) => setTimeout(resolve, n));
}

async function* foo(): AsyncIteratbleIterator<number> {
  await defer(10);
  yield 10;
  await defer(30);
  yield 40;
  await defer(30);
  yield 70;
}

async function* bar(): AsyncIterableIterator<number> {
  await defer(20);
  yield 20;
  await defer(30);
  yield 50;
  await defer(30);
  yield 80;
}

async function* baz(): AsyncIterableIterator<number> {
  await defer(30);
  yield 30;
  await defer(30);
  yield 60;
  await defer(30);
  yield 90;
}

test("mux returns an async iterator that merges the given async iterators.", async () => {
  const iter = mux(foo(), bar(), baz());
  assertEquals(await iter.next(), { done: false, value: 10 });
  assertEquals(await iter.next(), { done: false, value: 20 });
  assertEquals(await iter.next(), { done: false, value: 30 });
  assertEquals(await iter.next(), { done: false, value: 40 });
  assertEquals(await iter.next(), { done: false, value: 50 });
  assertEquals(await iter.next(), { done: false, value: 60 });
  assertEquals(await iter.next(), { done: false, value: 70 });
  assertEquals(await iter.next(), { done: false, value: 80 });
  assertEquals(await iter.next(), { done: false, value: 90 });
  assertEquals(await iter.next(), { done: true, value: undefined });
});

test("mux returns an promise which resolves with the first item from the merged async iterators.", async () => {
  const iter = mux(foo(), bar(), baz());
  assertEquals(await iter, 10);
  assertEquals(await iter, 20);
  assertEquals(await iter, 30);
  assertEquals(await iter, 40);
  assertEquals(await iter, 50);
  assertEquals(await iter, 60);
  assertEquals(await iter, 70);
  assertEquals(await iter, 80);
  assertEquals(await iter, 90);
  assertThrowsAsync(async () => {
    await iter;
  });
});
