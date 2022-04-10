// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// Utility for representing n-tuple
type Tuple<T, N extends number> = N extends N
  ? number extends N ? T[] : TupleOf<T, N, []>
  : never;
type TupleOf<T, N extends number, R extends unknown[]> = R["length"] extends N
  ? R
  : TupleOf<T, N, [T, ...R]>;

const noop = () => {};

class AsyncIterableClone<T> implements AsyncIterable<T> {
  currentPromise: Promise<IteratorResult<T>>;
  resolveCurrent: (x: Promise<IteratorResult<T>>) => void = noop;
  consumed: Promise<void>;
  consume: () => void = noop;

  constructor() {
    this.currentPromise = new Promise<IteratorResult<T>>((resolve) => {
      this.resolveCurrent = resolve;
    });
    this.consumed = new Promise<void>((resolve) => {
      this.consume = resolve;
    });
  }

  reset() {
    this.currentPromise = new Promise<IteratorResult<T>>((resolve) => {
      this.resolveCurrent = resolve;
    });
    this.consumed = new Promise<void>((resolve) => {
      this.consume = resolve;
    });
  }

  async next(): Promise<IteratorResult<T>> {
    const res = await this.currentPromise;
    this.consume();
    this.reset();
    return res;
  }

  async push(res: Promise<IteratorResult<T>>): Promise<void> {
    this.resolveCurrent(res);
    // Wait until current promise is consumed and next item is requested.
    await this.consumed;
  }

  [Symbol.asyncIterator](): AsyncIterator<T> {
    return this;
  }
}

/**
 * Branches the given async iterable into the n branches.
 *
 * Example:
 *
 * ```ts
 *     import { tee } from "./tee.ts";
 *
 *     const gen = async function* gen() {
 *       yield 1;
 *       yield 2;
 *       yield 3;
 *     }
 *
 *     const [branch1, branch2] = tee(gen());
 *
 *     (async () => {
 *       for await (const n of branch1) {
 *         console.log(n); // => 1, 2, 3
 *       }
 *     })();
 *
 *     (async () => {
 *       for await (const n of branch2) {
 *         console.log(n); // => 1, 2, 3
 *       }
 *     })();
 * ```
 */
export function tee<T, N extends number = 2>(
  src: AsyncIterable<T>,
  n: N = 2 as N,
): Tuple<AsyncIterable<T>, N> {
  const clones: Tuple<AsyncIterableClone<T>, N> = Array.from({ length: n }).map(
    () => new AsyncIterableClone(),
    // deno-lint-ignore no-explicit-any
  ) as any;
  (async () => {
    const iter = src[Symbol.asyncIterator]();
    await Promise.resolve();
    while (true) {
      const res = iter.next();
      await Promise.all(clones.map((c) => c.push(res)));
      if ((await res).done) {
        break;
      }
    }
  })().catch((e) => {
    console.error(e);
  });
  return clones;
}
