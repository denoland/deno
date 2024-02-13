// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Splits the given array into chunks of the given size and returns them.
 *
 * @example
 * ```ts
 * import { chunk } from "https://deno.land/std@$STD_VERSION/collections/chunk.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const words = [
 *   "lorem",
 *   "ipsum",
 *   "dolor",
 *   "sit",
 *   "amet",
 *   "consetetur",
 *   "sadipscing",
 * ];
 * const chunks = chunk(words, 3);
 *
 * assertEquals(
 *   chunks,
 *   [
 *     ["lorem", "ipsum", "dolor"],
 *     ["sit", "amet", "consetetur"],
 *     ["sadipscing"],
 *   ],
 * );
 * ```
 */
export function chunk<T>(array: readonly T[], size: number): T[][] {
  if (size <= 0 || !Number.isInteger(size)) {
    throw new Error(
      `Expected size to be an integer greater than 0 but found ${size}`,
    );
  }

  if (array.length === 0) {
    return [];
  }

  const ret = Array.from<T[]>({ length: Math.ceil(array.length / size) });
  let readIndex = 0;
  let writeIndex = 0;

  while (readIndex < array.length) {
    ret[writeIndex] = array.slice(readIndex, readIndex + size);

    writeIndex += 1;
    readIndex += size;
  }

  return ret;
}
