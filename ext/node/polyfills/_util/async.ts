// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is vendored from std/async/delay.ts
// (with some modifications)

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

/** Resolve a Promise after a given amount of milliseconds. */
export function delay(
  ms: number,
  options: { signal?: AbortSignal } = {},
): Promise<void> {
  const { signal } = options;
  if (signal?.aborted) {
    return Promise.reject(new DOMException("Delay was aborted.", "AbortError"));
  }
  return new Promise((resolve, reject) => {
    const abort = () => {
      clearTimeout(i);
      reject(new DOMException("Delay was aborted.", "AbortError"));
    };
    const done = () => {
      signal?.removeEventListener("abort", abort);
      resolve();
    };
    const i = setTimeout(done, ms);
    signal?.addEventListener("abort", abort, { once: true });
  });
}
