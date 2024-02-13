// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Builds two separate arrays from the given array of 2-tuples, with the first
 * returned array holding all first tuple elements and the second one holding
 * all the second elements.
 *
 * ```ts
 * import { unzip } from "https://deno.land/std@$STD_VERSION/collections/unzip.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const parents = [
 *   ["Maria", "Jeff"],
 *   ["Anna", "Kim"],
 *   ["John", "Leroy"],
 * ] as [string, string][];
 *
 * const [moms, dads] = unzip(parents);
 *
 * assertEquals(moms, ["Maria", "Anna", "John"]);
 * assertEquals(dads, ["Jeff", "Kim", "Leroy"]);
 * ```
 */
export function unzip<T, U>(pairs: readonly [T, U][]): [T[], U[]] {
  const { length } = pairs;
  const ret: [T[], U[]] = [
    Array<T>(length),
    Array<U>(length),
  ];

  pairs.forEach(([first, second], index) => {
    ret[0][index] = first;
    ret[1][index] = second;
  });

  return ret;
}
