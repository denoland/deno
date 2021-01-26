// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { delay } from "./delay.ts";
import { pooledMap } from "./pool.ts";
import {
  assert,
  assertEquals,
  assertStringIncludes,
  assertThrowsAsync,
} from "../testing/asserts.ts";

Deno.test("[async] pooledMap", async function (): Promise<void> {
  const start = new Date();
  const results = pooledMap(
    2,
    [1, 2, 3],
    (i) => new Promise((r) => setTimeout(() => r(i), 1000)),
  );
  for await (const value of results) {
    console.log(value);
  }
  const diff = new Date().getTime() - start.getTime();
  assert(diff >= 2000);
  assert(diff < 3000);
});

Deno.test("[async] pooledMap errors", async function (): Promise<void> {
  async function mapNumber(n: number): Promise<number> {
    if (n <= 2) {
      throw new Error(`Bad number: ${n}`);
    }
    await delay(100);
    return n;
  }
  const mappedNumbers: number[] = [];
  const error = await assertThrowsAsync(async () => {
    for await (const m of pooledMap(3, [1, 2, 3, 4], mapNumber)) {
      mappedNumbers.push(m);
    }
  }, AggregateError) as AggregateError;
  assertEquals(mappedNumbers, [3]);
  assertEquals(error.errors.length, 2);
  assertStringIncludes(error.errors[0].stack, "Error: Bad number: 1");
  assertStringIncludes(error.errors[1].stack, "Error: Bad number: 2");
});
