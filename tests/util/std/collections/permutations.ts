// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Builds all possible orders of all elements in the given array
 * Ignores equality of elements, meaning this will always return the same
 * number of permutations for a given length of input.
 *
 * @example
 * ```ts
 * import { permutations } from "https://deno.land/std@$STD_VERSION/collections/permutations.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const numbers = [ 1, 2 ];
 * const windows = permutations(numbers);
 *
 * assertEquals(windows, [
 *   [ 1, 2 ],
 *   [ 2, 1 ],
 * ]);
 * ```
 */
export function permutations<T>(inputArray: Iterable<T>): T[][] {
  const ret: T[][] = [];

  const array = [...inputArray];

  const k = array.length;

  if (k === 0) {
    return ret;
  }

  // Heap's Algorithm
  const c = new Array<number>(k).fill(0);

  ret.push([...array]);

  let i = 1;

  while (i < k) {
    if (c[i] < i) {
      if (i % 2 === 0) {
        [array[0], array[i]] = [array[i], array[0]];
      } else {
        [array[c[i]], array[i]] = [array[i], array[c[i]]];
      }

      ret.push([...array]);

      c[i] += 1;
      i = 1;
    } else {
      c[i] = 0;
      i += 1;
    }
  }

  return ret;
}
