// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Generates sliding views of the given array of the given size and returns a
 * new array containing all of them.
 *
 * If step is set, each window will start that many elements after the last
 * window's start. (Default: 1)
 *
 * If partial is set, windows will be generated for the last elements of the
 * collection, resulting in some undefined values if size is greater than 1.
 *
 * @example
 * ```ts
 * import { slidingWindows } from "https://deno.land/std@$STD_VERSION/collections/sliding_windows.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 * const numbers = [1, 2, 3, 4, 5];
 *
 * const windows = slidingWindows(numbers, 3);
 * assertEquals(windows, [
 *   [1, 2, 3],
 *   [2, 3, 4],
 *   [3, 4, 5],
 * ]);
 *
 * const windowsWithStep = slidingWindows(numbers, 3, { step: 2 });
 * assertEquals(windowsWithStep, [
 *   [1, 2, 3],
 *   [3, 4, 5],
 * ]);
 *
 * const windowsWithPartial = slidingWindows(numbers, 3, { partial: true });
 * assertEquals(windowsWithPartial, [
 *   [1, 2, 3],
 *   [2, 3, 4],
 *   [3, 4, 5],
 *   [4, 5],
 *   [5],
 * ]);
 * ```
 */
export function slidingWindows<T>(
  array: readonly T[],
  size: number,
  { step = 1, partial = false }: {
    /**
     * If step is set, each window will start that many elements after the last
     * window's start.
     *
     * @default {1}
     */
    step?: number;
    /**
     * If partial is set, windows will be generated for the last elements of the
     * collection, resulting in some undefined values if size is greater than 1.
     *
     * @default {false}
     */
    partial?: boolean;
  } = {},
): T[][] {
  if (
    !Number.isInteger(size) || !Number.isInteger(step) || size <= 0 || step <= 0
  ) {
    throw new RangeError("Both size and step must be positive integer.");
  }

  /** length of the return array */
  const length = Math.floor((array.length - (partial ? 1 : size)) / step + 1);

  const result = [];
  for (let i = 0; i < length; i++) {
    result.push(array.slice(i * step, i * step + size));
  }
  return result;
}
