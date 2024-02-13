// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns all elements in the given array after the last element that does not
 * match the given predicate.
 *
 * @example
 * ```ts
 * import { takeLastWhile } from "https://deno.land/std@$STD_VERSION/collections/take_last_while.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const arr = [1, 2, 3, 4, 5, 6];
 *
 * assertEquals(
 *   takeLastWhile(arr, (i) => i > 4),
 *   [5, 6],
 * );
 * ```
 */
export function takeLastWhile<T>(
  array: readonly T[],
  predicate: (el: T) => boolean,
): T[] {
  let offset = array.length;
  while (0 < offset && predicate(array[offset - 1])) offset--;

  return array.slice(offset, array.length);
}
