// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Applies the given transformer to all entries in the given record and returns
 * a new record containing the results.
 *
 * @example
 * ```ts
 * import { mapEntries } from "https://deno.land/std@$STD_VERSION/collections/map_entries.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const usersById = {
 *   "a2e": { name: "Kim", age: 22 },
 *   "dfe": { name: "Anna", age: 31 },
 *   "34b": { name: "Tim", age: 58 },
 * } as const;
 * const agesByNames = mapEntries(usersById, ([id, { name, age }]) => [name, age]);
 *
 * assertEquals(
 *   agesByNames,
 *   {
 *     "Kim": 22,
 *     "Anna": 31,
 *     "Tim": 58,
 *   },
 * );
 * ```
 */
export function mapEntries<T, O>(
  record: Readonly<Record<string, T>>,
  transformer: (entry: [string, T]) => [string, O],
): Record<string, O> {
  const ret: Record<string, O> = {};
  const entries = Object.entries(record);

  for (const entry of entries) {
    const [mappedKey, mappedValue] = transformer(entry);

    ret[mappedKey] = mappedValue;
  }

  return ret;
}
