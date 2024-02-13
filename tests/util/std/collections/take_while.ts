// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns all elements in the given collection until the first element that
 * does not match the given predicate.
 *
 * @example
 * ```ts
 * import { takeWhile } from "https://deno.land/std@$STD_VERSION/collections/take_while.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const arr = [1, 2, 3, 4, 5, 6];
 *
 * assertEquals(
 *   takeWhile(arr, (i) => i !== 4),
 *   [1, 2, 3],
 * );
 * ```
 */
export function takeWhile<T>(
  array: readonly T[],
  predicate: (el: T) => boolean,
): T[] {
  let offset = 0;
  const length = array.length;

  while (length > offset && predicate(array[offset])) {
    offset++;
  }

  return array.slice(0, offset);
}
