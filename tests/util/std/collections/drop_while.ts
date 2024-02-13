// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns a new array that drops all elements in the given collection until the
 * first element that does not match the given predicate.
 *
 * @example
 * ```ts
 * import { dropWhile } from "https://deno.land/std@$STD_VERSION/collections/drop_while.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const numbers = [3, 2, 5, 2, 5];
 * const dropWhileNumbers = dropWhile(numbers, (i) => i !== 2);
 *
 * assertEquals(dropWhileNumbers, [2, 5, 2, 5]);
 * ```
 */
export function dropWhile<T>(
  array: readonly T[],
  predicate: (el: T) => boolean,
): T[] {
  let offset = 0;
  const length = array.length;

  while (length > offset && predicate(array[offset])) {
    offset++;
  }

  return array.slice(offset, length);
}
