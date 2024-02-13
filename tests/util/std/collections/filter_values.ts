// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns a new record with all entries of the given record except the ones
 * that have a value that does not match the given predicate.
 *
 * @example
 * ```ts
 * import { filterValues } from "https://deno.land/std@$STD_VERSION/collections/filter_values.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const people = {
 *   "Arnold": 37,
 *   "Sarah": 7,
 *   "Kim": 23,
 * };
 * const adults = filterValues(people, (it) => it >= 18);
 *
 * assertEquals(
 *   adults,
 *   {
 *     "Arnold": 37,
 *     "Kim": 23,
 *   },
 * );
 * ```
 */
export function filterValues<T>(
  record: Readonly<Record<string, T>>,
  predicate: (value: T) => boolean,
): Record<string, T> {
  const ret: Record<string, T> = {};
  const entries = Object.entries(record);

  for (const [key, value] of entries) {
    if (predicate(value)) {
      ret[key] = value;
    }
  }

  return ret;
}
