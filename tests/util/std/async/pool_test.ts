// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { delay } from "./delay.ts";
import { ERROR_WHILE_MAPPING_MESSAGE, pooledMap } from "./pool.ts";
import {
  assert,
  assertEquals,
  assertRejects,
  assertStringIncludes,
} from "../assert/mod.ts";

Deno.test("[async] pooledMap", async function () {
  const start = new Date();
  const results = pooledMap(
    2,
    [1, 2, 3],
    (i) => new Promise<number>((r) => setTimeout(() => r(i), 1000)),
  );
  const array = await Array.fromAsync(results);
  assertEquals(array, [1, 2, 3]);
  const diff = new Date().getTime() - start.getTime();
  assert(diff >= 2000);
  assert(diff < 3000);
});

Deno.test("[async] pooledMap errors", async () => {
  async function mapNumber(n: number): Promise<number> {
    if (n <= 2) {
      throw new Error(`Bad number: ${n}`);
    }
    await delay(100);
    return n;
  }
  const mappedNumbers: number[] = [];
  const error = await assertRejects(
    async () => {
      for await (const m of pooledMap(3, [1, 2, 3, 4], mapNumber)) {
        mappedNumbers.push(m);
      }
    },
    AggregateError,
    ERROR_WHILE_MAPPING_MESSAGE,
  );
  assertEquals(error.errors.length, 2);
  assertStringIncludes(error.errors[0].stack, "Error: Bad number: 1");
  assertStringIncludes(error.errors[1].stack, "Error: Bad number: 2");
  assertEquals(mappedNumbers, [3]);
});

Deno.test("pooledMap returns ordered items", async () => {
  function getRandomInt(min: number, max: number): number {
    min = Math.ceil(min);
    max = Math.floor(max);
    return Math.floor(Math.random() * (max - min) + min); //The maximum is exclusive and the minimum is inclusive
  }

  const results = pooledMap(
    2,
    [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    (i) =>
      new Promise<number>((r) =>
        setTimeout(() => r(i), getRandomInt(5, 20) * 100)
      ),
  );

  const returned = await Array.fromAsync(results);
  assertEquals(returned, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
});

Deno.test("[async] pooledMap (browser compat)", async function () {
  // Simulates the environment where Symbol.asyncIterator is not available
  const asyncIterFunc = ReadableStream.prototype[Symbol.asyncIterator];
  // deno-lint-ignore no-explicit-any
  delete (ReadableStream.prototype as any)[Symbol.asyncIterator];
  try {
    const results = pooledMap(
      2,
      [1, 2, 3],
      (i) => new Promise<number>((r) => setTimeout(() => r(i), 100)),
    );
    const array = await Array.fromAsync(results);
    assertEquals(array, [1, 2, 3]);
  } finally {
    ReadableStream.prototype[Symbol.asyncIterator] = asyncIterFunc;
  }
});
