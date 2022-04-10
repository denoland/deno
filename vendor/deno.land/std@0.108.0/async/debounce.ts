// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/**
 * A debounced function that will be delayed by a given `wait`
 * time in milliseconds. If the method is called again before
 * the timeout expires, the previous call will be aborted.
 */
export interface DebouncedFunction<T extends Array<unknown>> {
  (...args: T): void;
  /** Clears the debounce timeout and omits calling the debounced function. */
  clear(): void;
  /** Clears the debounce timeout and calls the debounced function immediately. */
  flush(): void;
  /** Returns a boolean wether a debounce call is pending or not. */
  readonly pending: boolean;
}

/**
 * Creates a debounced function that delays the given `func`
 * by a given `wait` time in milliseconds. If the method is called
 * again before the timeout expires, the previous call will be
 * aborted.
 *
 * ```
 * import { debounce } from "./debounce.ts";
 *
 * const log = debounce(
 *   (event: Deno.FsEvent) =>
 *     console.log("[%s] %s", event.kind, event.paths[0]),
 *   200,
 * );
 *
 * for await (const event of Deno.watchFs("./")) {
 *   log(event);
 * }
 * ```
 *
 * @param fn    The function to debounce.
 * @param wait  The time in milliseconds to delay the function.
 */
// deno-lint-ignore no-explicit-any
export function debounce<T extends Array<any>>(
  fn: (this: DebouncedFunction<T>, ...args: T) => void,
  wait: number,
): DebouncedFunction<T> {
  let timeout: number | null = null;
  let flush: (() => void) | null = null;

  const debounced: DebouncedFunction<T> = ((...args: T): void => {
    debounced.clear();
    flush = (): void => {
      debounced.clear();
      fn.call(debounced, ...args);
    };
    timeout = setTimeout(flush, wait);
  }) as DebouncedFunction<T>;

  debounced.clear = (): void => {
    if (typeof timeout === "number") {
      clearTimeout(timeout);
      timeout = null;
      flush = null;
    }
  };

  debounced.flush = (): void => {
    flush?.();
  };

  Object.defineProperty(debounced, "pending", {
    get: () => typeof timeout === "number",
  });

  return debounced;
}
