// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns an array excluding all given values.
 *
 * @example
 * ```ts
 * import { withoutAll } from "https://deno.land/std@$STD_VERSION/collections/without_all.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const withoutList = withoutAll([2, 1, 2, 3], [1, 2]);
 *
 * assertEquals(withoutList, [3]);
 * ```
 */
export function withoutAll<T>(array: readonly T[], values: readonly T[]): T[] {
  const toExclude = new Set(values);
  return array.filter((it) => !toExclude.has(it));
}
