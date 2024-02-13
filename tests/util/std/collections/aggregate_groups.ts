// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { mapEntries } from "./map_entries.ts";

/**
 * Applies the given aggregator to each group in the given grouping, returning the
 * results together with the respective group keys
 *
 * @template T input type of an item in a group in the given grouping.
 * @template A type of the accumulator value, which will match the returned record's values.
 * @example
 * ```ts
 * import { aggregateGroups } from "https://deno.land/std@$STD_VERSION/collections/aggregate_groups.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const foodProperties = {
 *   "Curry": ["spicy", "vegan"],
 *   "Omelette": ["creamy", "vegetarian"],
 * };
 * const descriptions = aggregateGroups(
 *   foodProperties,
 *   (current, key, first, acc) => {
 *     if (first) {
 *       return `${key} is ${current}`;
 *     }
 *
 *     return `${acc} and ${current}`;
 *   },
 * );
 *
 * assertEquals(descriptions, {
 *   "Curry": "Curry is spicy and vegan",
 *   "Omelette": "Omelette is creamy and vegetarian",
 * });
 * ```
 */
export function aggregateGroups<T, A>(
  record: Readonly<Record<string, ReadonlyArray<T>>>,
  aggregator: (current: T, key: string, first: boolean, accumulator?: A) => A,
): Record<string, A> {
  return mapEntries(
    record,
    ([key, values]) => [
      key,
      // Need the type assertions here because the reduce type does not support the type transition we need
      values.reduce(
        (accumulator, current, currentIndex) =>
          aggregator(current, key, currentIndex === 0, accumulator),
        undefined as A | undefined,
      ) as A,
    ],
  );
}
