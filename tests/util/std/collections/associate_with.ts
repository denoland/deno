// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Builds a new Record using the given array as keys and choosing a value for
 * each key using the given selector. If any of two pairs would have the same
 * value the latest on will be used (overriding the ones before it).
 *
 * @example
 * ```ts
 * import { associateWith } from "https://deno.land/std@$STD_VERSION/collections/associate_with.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const names = ["Kim", "Lara", "Jonathan"];
 * const namesToLength = associateWith(names, (it) => it.length);
 *
 * assertEquals(namesToLength, {
 *   "Kim": 3,
 *   "Lara": 4,
 *   "Jonathan": 8,
 * });
 * ```
 */
export function associateWith<T>(
  array: Iterable<string>,
  selector: (key: string) => T,
): Record<string, T> {
  const ret: Record<string, T> = {};

  for (const element of array) {
    const selectedValue = selector(element);

    ret[element] = selectedValue;
  }

  return ret;
}
