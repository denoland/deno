// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/**
 * pooledMap transforms values from an (async) iterable into another async
 * iterable. The transforms are done concurrently, with a max concurrency
 * defined by the poolLimit.
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
    ): Promise<void> {
      controller.enqueue(await p);
    },
  });
  // Start processing items from the iterator
  (async (): Promise<void> => {
    const writer = res.writable.getWriter();
    const executing: Array<Promise<unknown>> = [];
    for await (const item of array) {
      const p = Promise.resolve().then(() => iteratorFn(item));
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
  })();
  return res.readable[Symbol.asyncIterator]();
}
