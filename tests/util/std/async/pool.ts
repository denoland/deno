// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

export const ERROR_WHILE_MAPPING_MESSAGE = "Threw while mapping.";

/**
 * pooledMap transforms values from an (async) iterable into another async
 * iterable. The transforms are done concurrently, with a max concurrency
 * defined by the poolLimit.
 *
 * If an error is thrown from `iterableFn`, no new transformations will begin.
 * All currently executing transformations are allowed to finish and still
 * yielded on success. After that, the rejections among them are gathered and
 * thrown by the iterator in an `AggregateError`.
 *
 * @example
 * ```typescript
 * import { pooledMap } from "https://deno.land/std@$STD_VERSION/async/pool.ts";
 *
 * const results = pooledMap(
 *   2,
 *   [1, 2, 3],
 *   (i) => new Promise((r) => setTimeout(() => r(i), 1000)),
 * );
 *
 * for await (const value of results) {
 *   // ...
 * }
 * ```
 *
 * @param poolLimit The maximum count of items being processed concurrently.
 * @param array The input array for mapping.
 * @param iteratorFn The function to call for every item of the array.
 */
export function pooledMap<T, R>(
  poolLimit: number,
  array: Iterable<T> | AsyncIterable<T>,
  iteratorFn: (data: T) => Promise<R>,
): AsyncIterableIterator<R> {
  // Create the async iterable that is returned from this function.
  const res = new TransformStream<Promise<R>, R>({
    async transform(
      p: Promise<R>,
      controller: TransformStreamDefaultController<R>,
    ) {
      try {
        const s = await p;
        controller.enqueue(s);
      } catch (e) {
        if (
          e instanceof AggregateError &&
          e.message === ERROR_WHILE_MAPPING_MESSAGE
        ) {
          controller.error(e as unknown);
        }
      }
    },
  });
  // Start processing items from the iterator
  (async () => {
    const writer = res.writable.getWriter();
    const executing: Array<Promise<unknown>> = [];
    try {
      for await (const item of array) {
        const p = Promise.resolve().then(() => iteratorFn(item));
        // Only write on success. If we `writer.write()` a rejected promise,
        // that will end the iteration. We don't want that yet. Instead let it
        // fail the race, taking us to the catch block where all currently
        // executing jobs are allowed to finish and all rejections among them
        // can be reported together.
        writer.write(p);
        const e: Promise<unknown> = p.then(() =>
          executing.splice(executing.indexOf(e), 1)
        );
        executing.push(e);
        if (executing.length >= poolLimit) {
          await Promise.race(executing);
        }
      }
      // Wait until all ongoing events have processed, then close the writer.
      await Promise.all(executing);
      writer.close();
    } catch {
      const errors = [];
      for (const result of await Promise.allSettled(executing)) {
        if (result.status === "rejected") {
          errors.push(result.reason);
        }
      }
      writer.write(Promise.reject(
        new AggregateError(errors, ERROR_WHILE_MAPPING_MESSAGE),
      )).catch(() => {});
    }
  })();
  // Feature test until browser coverage is adequate
  return Symbol.asyncIterator in res.readable &&
      typeof res.readable[Symbol.asyncIterator] === "function"
    ? (res.readable[Symbol.asyncIterator] as () => AsyncIterableIterator<R>)()
    : (async function* () {
      const reader = res.readable.getReader();
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        yield value;
      }
      reader.releaseLock();
    })();
}
