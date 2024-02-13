// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns a tuple of two arrays with the first one containing all elements in
 * the given array that match the given predicate and the second one containing
 * all that do not.
 *
 * @example
 * ```ts
 * import { partition } from "https://deno.land/std@$STD_VERSION/collections/partition.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const numbers = [5, 6, 7, 8, 9];
 * const [even, odd] = partition(numbers, (it) => it % 2 === 0);
 *
 * assertEquals(even, [6, 8]);
 * assertEquals(odd, [5, 7, 9]);
 * ```
 */
export function partition<T>(
  array: Iterable<T>,
  predicate: (el: T) => boolean,
): [T[], T[]];
/**
 * Returns a tuple of two arrays with the first one containing all elements in
 * the given array that match the given predicate and the second one containing
 * all that do not.
 *
 * @example
 * ```ts
 * import { partition } from "https://deno.land/std@$STD_VERSION/collections/partition.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const numbers = [5, 6, 7, 8, 9];
 * const [even, odd] = partition(numbers, (it) => it % 2 === 0);
 *
 * assertEquals(even, [6, 8]);
 * assertEquals(odd, [5, 7, 9]);
 * ```
 */
export function partition<T, U extends T>(
  array: Iterable<T>,
  predicate: (el: T) => el is U,
): [U[], Exclude<T, U>[]];
export function partition(
  array: Iterable<unknown>,
  predicate: (el: unknown) => boolean,
): [unknown[], unknown[]] {
  const matches: Array<unknown> = [];
  const rest: Array<unknown> = [];

  for (const element of array) {
    if (predicate(element)) {
      matches.push(element);
    } else {
      rest.push(element);
    }
  }

  return [matches, rest];
}
