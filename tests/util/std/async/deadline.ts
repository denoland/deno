// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { delay } from "./delay.ts";

export interface DeadlineOptions {
  /** Signal used to abort the deadline. */
  signal?: AbortSignal;
}

export class DeadlineError extends Error {
  constructor() {
    super("Deadline");
    this.name = this.constructor.name;
  }
}

/**
 * Create a promise which will be rejected with {@linkcode DeadlineError} when a given delay is exceeded.
 *
 * NOTE: Prefer to use `AbortSignal.timeout` instead for the APIs accept `AbortSignal`.
 *
 * @example
 * ```typescript
 * import { deadline } from "https://deno.land/std@$STD_VERSION/async/deadline.ts";
 * import { delay } from "https://deno.land/std@$STD_VERSION/async/delay.ts";
 *
 * const delayedPromise = delay(1000);
 * // Below throws `DeadlineError` after 10 ms
 * const result = await deadline(delayedPromise, 10);
 * ```
 */
export function deadline<T>(
  p: Promise<T>,
  ms: number,
  options: DeadlineOptions = {},
): Promise<T> {
  const controller = new AbortController();
  const { signal } = options;
  if (signal?.aborted) {
    return Promise.reject(new DeadlineError());
  }
  signal?.addEventListener("abort", () => controller.abort(signal.reason));
  const d = delay(ms, { signal: controller.signal })
    .catch(() => {}) // Do NOTHING on abort.
    .then(() => Promise.reject(new DeadlineError()));
  return Promise.race([p.finally(() => controller.abort()), d]);
}
