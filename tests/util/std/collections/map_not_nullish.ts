// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns a new array, containing all elements in the given array transformed
 * using the given transformer, except the ones that were transformed to `null`
 * or `undefined`.
 *
 * @example
 * ```ts
 * import { mapNotNullish } from "https://deno.land/std@$STD_VERSION/collections/map_not_nullish.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const people = [
 *   { middleName: null },
 *   { middleName: "William" },
 *   { middleName: undefined },
 *   { middleName: "Martha" },
 * ];
 * const foundMiddleNames = mapNotNullish(people, (it) => it.middleName);
 *
 * assertEquals(foundMiddleNames, ["William", "Martha"]);
 * ```
 */
export function mapNotNullish<T, O>(
  array: Iterable<T>,
  transformer: (el: T) => O,
): NonNullable<O>[] {
  const ret: NonNullable<O>[] = [];

  for (const element of array) {
    const transformedElement = transformer(element);

    if (transformedElement !== undefined && transformedElement !== null) {
      ret.push(transformedElement as NonNullable<O>);
    }
  }

  return ret;
}
