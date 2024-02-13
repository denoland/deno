// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns all distinct elements in the given array, preserving order by first
 * occurrence.
 *
 * @example
 * ```ts
 * import { distinct } from "https://deno.land/std@$STD_VERSION/collections/distinct.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const numbers = [3, 2, 5, 2, 5];
 * const distinctNumbers = distinct(numbers);
 *
 * assertEquals(distinctNumbers, [3, 2, 5]);
 * ```
 */
export function distinct<T>(array: Iterable<T>): T[] {
  const set = new Set(array);

  return Array.from(set);
}
