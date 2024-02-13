// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertRejects } from "../assert/mod.ts";
import { MuxAsyncIterator } from "./mux_async_iterator.ts";

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

async function* genThrows(): AsyncIterableIterator<number> {
  yield 7;
  throw new Error("something went wrong");
}

class CustomAsyncIterable {
  [Symbol.asyncIterator]() {
    return gen123();
  }
}

Deno.test("[async] MuxAsyncIterator", async function () {
  const mux = new MuxAsyncIterator<number>();
  mux.add(gen123());
  mux.add(gen456());
  const results = new Set(await Array.fromAsync(mux));
  assertEquals(results.size, 6);
  assertEquals(results, new Set([1, 2, 3, 4, 5, 6]));
});

Deno.test("[async] MuxAsyncIterator takes async iterable as source", async function () {
  const mux = new MuxAsyncIterator<number>();
  mux.add(new CustomAsyncIterable());
  const results = new Set(await Array.fromAsync(mux));
  assertEquals(results.size, 3);
  assertEquals(results, new Set([1, 2, 3]));
});

Deno.test({
  name: "[async] MuxAsyncIterator throws when the source throws",
  async fn() {
    const mux = new MuxAsyncIterator<number>();
    mux.add(gen123());
    mux.add(genThrows());
    await assertRejects(
      async () => await Array.fromAsync(mux),
      Error,
      "something went wrong",
    );
  },
});
