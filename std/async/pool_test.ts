import { pooledMap } from "./pool.ts";
import { assert } from "../testing/asserts.ts";

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

export {};
