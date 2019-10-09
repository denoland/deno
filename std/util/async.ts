// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// TODO(ry) It'd be better to make Deferred a class that inherits from
// Promise, rather than an interface. This is possible in ES2016, however
// typescript produces broken code when targeting ES5 code.
// See https://github.com/Microsoft/TypeScript/issues/15202
// At the time of writing, the github issue is closed but the problem remains.
export interface Deferred<T> extends Promise<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  reject: (reason?: any) => void;
}

/** Creates a Promise with the `reject` and `resolve` functions
 * placed as methods on the promise object itself. It allows you to do:
 *
 *     const p = deferred<number>();
 *     // ...
 *     p.resolve(42);
 */
export function deferred<T>(): Deferred<T> {
  let methods;
  const promise = new Promise<T>((resolve, reject): void => {
    methods = { resolve, reject };
  });
  return Object.assign(promise, methods)! as Deferred<T>;
}

interface TaggedYieldedValue<T> {
  iterator: AsyncIterableIterator<T>;
  value: T;
}

/** The MuxAsyncIterator class multiplexes multiple async iterators into a
 * single stream. It currently makes a few assumptions:
 * - The iterators do not throw.
 * - The final result (the value returned and not yielded from the iterator)
 *   does not matter; if there is any, it is discarded.
 */
export class MuxAsyncIterator<T> implements AsyncIterable<T> {
  private iteratorCount = 0;
  private yields: Array<TaggedYieldedValue<T>> = [];
  private signal: Deferred<void> = deferred();

  add(iterator: AsyncIterableIterator<T>): void {
    ++this.iteratorCount;
    this.callIteratorNext(iterator);
  }

  private async callIteratorNext(
    iterator: AsyncIterableIterator<T>
  ): Promise<void> {
    const { value, done } = await iterator.next();
    if (done) {
      --this.iteratorCount;
    } else {
      this.yields.push({ iterator, value });
    }
    this.signal.resolve();
  }

  async *iterate(): AsyncIterableIterator<T> {
    while (this.iteratorCount > 0) {
      // Sleep until any of the wrapped iterators yields.
      await this.signal;

      // Note that while we're looping over `yields`, new items may be added.
      for (let i = 0; i < this.yields.length; i++) {
        const { iterator, value } = this.yields[i];
        yield value;
        this.callIteratorNext(iterator);
      }

      // Clear the `yields` list and reset the `signal` promise.
      this.yields.length = 0;
      this.signal = deferred();
    }
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<T> {
    return this.iterate();
  }
}

/** Collects all Uint8Arrays from an AsyncIterable and retuns a single
 * Uint8Array with the concatenated contents of all the collected arrays.
 */
export async function collectUint8Arrays(
  it: AsyncIterable<Uint8Array>
): Promise<Uint8Array> {
  const chunks = [];
  let length = 0;
  for await (const chunk of it) {
    chunks.push(chunk);
    length += chunk.length;
  }
  if (chunks.length === 1) {
    // No need to copy.
    return chunks[0];
  }
  const collected = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    collected.set(chunk, offset);
    offset += chunk.length;
  }
  return collected;
}

// Delays the given milliseconds and resolves.
export function delay(ms: number): Promise<void> {
  return new Promise((res): number =>
    setTimeout((): void => {
      res();
    }, ms)
  );
}
