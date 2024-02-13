// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Applies the given transformer to all values in the given record and returns a
 * new record containing the resulting keys associated to the last value that
 * produced them.
 *
 * @example
 * ```ts
 * import { mapValues } from "https://deno.land/std@$STD_VERSION/collections/map_values.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const usersById = {
 *   "a5ec": { name: "Mischa" },
 *   "de4f": { name: "Kim" },
 * };
 * const namesById = mapValues(usersById, (it) => it.name);
 *
 * assertEquals(
 *   namesById,
 *   {
 *     "a5ec": "Mischa",
 *     "de4f": "Kim",
 *   },
 * );
 * ```
 */
export function mapValues<T, O>(
  record: Readonly<Record<string, T>>,
  transformer: (value: T) => O,
): Record<string, O> {
  const ret: Record<string, O> = {};
  const entries = Object.entries(record);

  for (const [key, value] of entries) {
    const mappedValue = transformer(value);

    ret[key] = mappedValue;
  }

  return ret;
}
