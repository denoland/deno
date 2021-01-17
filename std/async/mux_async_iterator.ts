// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { Deferred, deferred } from "./deferred.ts";

interface TaggedYieldedValue<T> {
  iterator: AsyncIterableIterator<T>;
  value: T;
}

/** The MuxAsyncIterator class multiplexes multiple async iterators into a
 * single stream. It currently makes an assumption:
 * - The final result (the value returned and not yielded from the iterator)
 *   does not matter; if there is any, it is discarded.
 */
export class MuxAsyncIterator<T> implements AsyncIterable<T> {
  private iteratorCount = 0;
  private yields: Array<TaggedYieldedValue<T>> = [];
  // deno-lint-ignore no-explicit-any
  private throws: any[] = [];
  private signal: Deferred<void> = deferred();

  add(iterator: AsyncIterableIterator<T>): void {
    ++this.iteratorCount;
    this.callIteratorNext(iterator);
  }

  private async callIteratorNext(
    iterator: AsyncIterableIterator<T>,
  ): Promise<void> {
    try {
      const { value, done } = await iterator.next();
      if (done) {
        --this.iteratorCount;
      } else {
        this.yields.push({ iterator, value });
      }
    } catch (e) {
      this.throws.push(e);
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

      if (this.throws.length) {
        for (const e of this.throws) {
          throw e;
        }
        this.throws.length = 0;
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
