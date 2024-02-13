// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/** Returns the index of the last occurrence of the needle array in the source
 * array, or -1 if it is not present.
 *
 * A start index can be specified as the third argument that begins the search
 * at that given index. The start index defaults to the end of the array.
 *
 * The complexity of this function is O(source.length * needle.length).
 *
 * ```ts
 * import { lastIndexOfNeedle } from "https://deno.land/std@$STD_VERSION/bytes/last_index_of_needle.ts";
 * const source = new Uint8Array([0, 1, 2, 1, 2, 1, 2, 3]);
 * const needle = new Uint8Array([1, 2]);
 * console.log(lastIndexOfNeedle(source, needle)); // 5
 * console.log(lastIndexOfNeedle(source, needle, 4)); // 3
 * ```
 */
export function lastIndexOfNeedle(
  source: Uint8Array,
  needle: Uint8Array,
  start = source.length - 1,
): number {
  if (start < 0) {
    return -1;
  }
  if (start >= source.length) {
    start = source.length - 1;
  }
  const e = needle[needle.length - 1];
  for (let i = start; i >= 0; i--) {
    if (source[i] !== e) continue;
    const pin = i;
    let matched = 1;
    let j = i;
    while (matched < needle.length) {
      j--;
      if (source[j] !== needle[needle.length - 1 - (pin - j)]) {
        break;
      }
      matched++;
    }
    if (matched === needle.length) {
      return pin - needle.length + 1;
    }
  }
  return -1;
}
