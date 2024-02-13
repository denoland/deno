// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

interface TaggedYieldedValue<T> {
  iterator: AsyncIterator<T>;
  value: T;
}

/**
 * The MuxAsyncIterator class multiplexes multiple async iterators into a single
 * stream. It currently makes an assumption that the final result (the value
 * returned and not yielded from the iterator) does not matter; if there is any
 * result, it is discarded.
 *
 * @example
 * ```typescript
 * import { MuxAsyncIterator } from "https://deno.land/std@$STD_VERSION/async/mod.ts";
 *
 * async function* gen123(): AsyncIterableIterator<number> {
 *   yield 1;
 *   yield 2;
 *   yield 3;
 * }
 *
 * async function* gen456(): AsyncIterableIterator<number> {
 *   yield 4;
 *   yield 5;
 *   yield 6;
 * }
 *
 * const mux = new MuxAsyncIterator<number>();
 * mux.add(gen123());
 * mux.add(gen456());
 * for await (const value of mux) {
 *   // ...
 * }
 * // ..
 * ```
 */
export class MuxAsyncIterator<T> implements AsyncIterable<T> {
  #iteratorCount = 0;
  #yields: Array<TaggedYieldedValue<T>> = [];
  // deno-lint-ignore no-explicit-any
  #throws: any[] = [];
  #signal = Promise.withResolvers<void>();

  add(iterable: AsyncIterable<T>) {
    ++this.#iteratorCount;
    this.#callIteratorNext(iterable[Symbol.asyncIterator]());
  }

  async #callIteratorNext(
    iterator: AsyncIterator<T>,
  ) {
    try {
      const { value, done } = await iterator.next();
      if (done) {
        --this.#iteratorCount;
      } else {
        this.#yields.push({ iterator, value });
      }
    } catch (e) {
      this.#throws.push(e);
    }
    this.#signal.resolve();
  }

  async *iterate(): AsyncIterableIterator<T> {
    while (this.#iteratorCount > 0) {
      // Sleep until any of the wrapped iterators yields.
      await this.#signal.promise;

      // Note that while we're looping over `yields`, new items may be added.
      for (let i = 0; i < this.#yields.length; i++) {
        const { iterator, value } = this.#yields[i];
        yield value;
        this.#callIteratorNext(iterator);
      }

      if (this.#throws.length) {
        for (const e of this.#throws) {
          throw e;
        }
        this.#throws.length = 0;
      }
      // Clear the `yields` list and reset the `signal` promise.
      this.#yields.length = 0;
      this.#signal = Promise.withResolvers<void>();
    }
  }

  [Symbol.asyncIterator](): AsyncIterator<T> {
    return this.iterate();
  }
}
