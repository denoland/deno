// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Transforms the given array into a Record, extracting the key of each element
 * using the given selector. If the selector produces the same key for multiple
 * elements, the latest one will be used (overriding the ones before it).
 *
 * @example
 * ```ts
 * import { associateBy } from "https://deno.land/std@$STD_VERSION/collections/associate_by.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const users = [
 *   { id: "a2e", userName: "Anna" },
 *   { id: "5f8", userName: "Arnold" },
 *   { id: "d2c", userName: "Kim" },
 * ];
 * const usersById = associateBy(users, (it) => it.id);
 *
 * assertEquals(usersById, {
 *   "a2e": { id: "a2e", userName: "Anna" },
 *   "5f8": { id: "5f8", userName: "Arnold" },
 *   "d2c": { id: "d2c", userName: "Kim" },
 * });
 * ```
 */
export function associateBy<T>(
  array: Iterable<T>,
  selector: (el: T) => string,
): Record<string, T> {
  const ret: Record<string, T> = {};

  for (const element of array) {
    const selectedValue = selector(element);

    ret[selectedValue] = element;
  }

  return ret;
}
