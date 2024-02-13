// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns the first element having the largest value according to the provided
 * comparator or undefined if there are no elements.
 *
 * The comparator is expected to work exactly like one passed to `Array.sort`,
 * which means that `comparator(a, b)` should return a negative number if `a < b`,
 * a positive number if `a > b` and `0` if `a === b`.
 *
 * @example
 * ```ts
 * import { maxWith } from "https://deno.land/std@$STD_VERSION/collections/max_with.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const people = ["Kim", "Anna", "John", "Arthur"];
 * const largestName = maxWith(people, (a, b) => a.length - b.length);
 *
 * assertEquals(largestName, "Arthur");
 * ```
 */
export function maxWith<T>(
  array: Iterable<T>,
  comparator: (a: T, b: T) => number,
): T | undefined {
  let max: T | undefined = undefined;
  let isFirst = true;

  for (const current of array) {
    if (isFirst || comparator(current, <T> max) > 0) {
      max = current;
      isFirst = false;
    }
  }

  return max;
}
