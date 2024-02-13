// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Make Promise abortable with the given signal.
 *
 * @example
 * ```typescript
 * import { abortable } from "https://deno.land/std@$STD_VERSION/async/mod.ts";
 * import { delay } from "https://deno.land/std@$STD_VERSION/async/mod.ts";
 *
 * const p = delay(1000);
 * const c = new AbortController();
 * setTimeout(() => c.abort(), 100);
 *
 * // Below throws `DOMException` after 100 ms
 * await abortable(p, c.signal);
 * ```
 */
export function abortable<T>(p: Promise<T>, signal: AbortSignal): Promise<T>;
/**
 * Make AsyncIterable abortable with the given signal.
 *
 * @example
 * ```typescript
 * import { abortable } from "https://deno.land/std@$STD_VERSION/async/mod.ts";
 * import { delay } from "https://deno.land/std@$STD_VERSION/async/mod.ts";
 *
 * const p = async function* () {
 *   yield "Hello";
 *   await delay(1000);
 *   yield "World";
 * };
 * const c = new AbortController();
 * setTimeout(() => c.abort(), 100);
 *
 * // Below throws `DOMException` after 100 ms
 * // and items become `["Hello"]`
 * const items: string[] = [];
 * for await (const item of abortable(p(), c.signal)) {
 *   items.push(item);
 * }
 * ```
 */
export function abortable<T>(
  p: AsyncIterable<T>,
  signal: AbortSignal,
): AsyncGenerator<T>;
export function abortable<T>(
  p: Promise<T> | AsyncIterable<T>,
  signal: AbortSignal,
): Promise<T> | AsyncIterable<T> {
  if (p instanceof Promise) {
    return abortablePromise(p, signal);
  } else {
    return abortableAsyncIterable(p, signal);
  }
}

/**
 * Make Promise abortable with the given signal.
 *
 * @example
 * ```typescript
 * import { abortablePromise } from "https://deno.land/std@$STD_VERSION/async/mod.ts";
 *
 * const request = fetch("https://example.com");
 *
 * const c = new AbortController();
 * setTimeout(() => c.abort(), 100);
 *
 * const p = abortablePromise(request, c.signal);
 *
 * // The below throws if the request didn't resolve in 100ms
 * await p;
 * ```
 */
export function abortablePromise<T>(
  p: Promise<T>,
  signal: AbortSignal,
): Promise<T> {
  if (signal.aborted) {
    return Promise.reject(createAbortError(signal.reason));
  }
  const { promise, reject } = Promise.withResolvers<never>();
  const abort = () => reject(createAbortError(signal.reason));
  signal.addEventListener("abort", abort, { once: true });
  return Promise.race([
    promise,
    p.finally(() => {
      signal.removeEventListener("abort", abort);
    }),
  ]);
}

/**
 * Make AsyncIterable abortable with the given signal.
 *
 * @example
 * ```typescript
 * import { abortableAsyncIterable } from "https://deno.land/std@$STD_VERSION/async/mod.ts";
 * import { delay } from "https://deno.land/std@$STD_VERSION/async/mod.ts";
 *
 * const p = async function* () {
 *   yield "Hello";
 *   await delay(1000);
 *   yield "World";
 * };
 * const c = new AbortController();
 * setTimeout(() => c.abort(), 100);
 *
 * // Below throws `DOMException` after 100 ms
 * // and items become `["Hello"]`
 * const items: string[] = [];
 * for await (const item of abortableAsyncIterable(p(), c.signal)) {
 *   items.push(item);
 * }
 * ```
 */
export async function* abortableAsyncIterable<T>(
  p: AsyncIterable<T>,
  signal: AbortSignal,
): AsyncGenerator<T> {
  if (signal.aborted) {
    throw createAbortError(signal.reason);
  }
  const { promise, reject } = Promise.withResolvers<never>();
  const abort = () => reject(createAbortError(signal.reason));
  signal.addEventListener("abort", abort, { once: true });

  const it = p[Symbol.asyncIterator]();
  while (true) {
    const { done, value } = await Promise.race([promise, it.next()]);
    if (done) {
      signal.removeEventListener("abort", abort);
      return;
    }
    yield value;
  }
}

// This `reason` comes from `AbortSignal` thus must be `any`.
// deno-lint-ignore no-explicit-any
function createAbortError(reason?: any): DOMException {
  return new DOMException(
    reason ? `Aborted: ${reason}` : "Aborted",
    "AbortError",
  );
}
