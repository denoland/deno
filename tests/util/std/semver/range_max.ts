// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { INVALID } from "./constants.ts";
import { sort } from "./sort.ts";
import type { SemVer, SemVerRange } from "./types.ts";
import { testRange } from "./test_range.ts";

/**
 * The maximum valid SemVer for a given range or INVALID
 * @param range The range to calculate the max for
 * @returns A valid SemVer or INVALID
 */
export function rangeMax(range: SemVerRange): SemVer | undefined {
  // For and's, you take the smallest max
  // For or's, you take the biggest max
  //[ [1 and 2] or [2 and 3] ] = [ 1 or 2 ] = 2
  return sort(
    range.ranges.map((r) =>
      sort(r.filter((c) => testRange(c.max, range)).map((c) => c.max)).shift()!
    ),
  ).filter((v) => v).pop() ?? INVALID;
}
