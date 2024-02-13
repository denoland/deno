// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { indexOfNeedle } from "./index_of_needle.ts";

/** Returns true if the source array contains the needle array, false otherwise.
 *
 * A start index can be specified as the third argument that begins the search
 * at that given index. The start index defaults to the beginning of the array.
 *
 * The complexity of this function is O(source.length * needle.length).
 *
 * ```ts
 * import { includesNeedle } from "https://deno.land/std@$STD_VERSION/bytes/includes_needle.ts";
 * const source = new Uint8Array([0, 1, 2, 1, 2, 1, 2, 3]);
 * const needle = new Uint8Array([1, 2]);
 * console.log(includesNeedle(source, needle)); // true
 * console.log(includesNeedle(source, needle, 6)); // false
 * ```
 */
export function includesNeedle(
  source: Uint8Array,
  needle: Uint8Array,
  start = 0,
): boolean {
  return indexOfNeedle(source, needle, start) !== -1;
}
