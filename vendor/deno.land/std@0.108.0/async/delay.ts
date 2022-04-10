// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

export interface DelayOptions {
  signal?: AbortSignal;
}

/* Resolves after the given number of milliseconds. */
export function delay(ms: number, options: DelayOptions = {}): Promise<void> {
  const { signal } = options;
  return new Promise((resolve, reject): void => {
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
