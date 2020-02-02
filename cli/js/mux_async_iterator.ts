// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/** Returns an async iterator which merges the given async iterators. */
export function mux<T>(
  ...iters: Array<AsyncIterator<T>>
): AsyncIterableIterator<T> & PromiseLike<T> {
  return new MuxAsyncIterator<T>(iters);
}

class MuxAsyncIterator<T> implements AsyncIterableIterator<T>, PromiseLike<T> {
  private nexts: Array<Promise<IteratorResult<T>>>;
  constructor(private iters: Array<AsyncIterator<T>>) {
    this.nexts = this.iters.map(iter => iter.next());
  }

  async next(): Promise<IteratorResult<T>> {
    while (this.iters.length > 0) {
      const { next } = await Promise.race(
        this.nexts.map(async next => {
          await next;
          return { next };
        })
      );
      const i = this.nexts.indexOf(next);
      const res = await next;

      if (res.done) {
        this.nexts.splice(i, 1);
        this.iters.splice(i, 1);
        continue;
      }

      if (!res.done) {
        this.nexts.splice(i, 1, this.iters[i].next());
        return { done: false, value: res.value };
      }
    }

    return { done: true, value: undefined };
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<T> {
    return this;
  }

  async then<S, P>(
    f: (t: T) => S | Promise<S>,
    g?: (e: Error) => P | Promise<P>
  ): Promise<S | P> {
    const { done, value } = await this.next();
    if (!done) {
      return f(value);
    }

    const e = new Error("All async iterators have already been finished.");

    if (g) {
      return g(e);
    }

    throw e;
  }
}
