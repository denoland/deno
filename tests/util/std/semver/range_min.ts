// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { INVALID } from "./constants.ts";
import { sort } from "./sort.ts";
import type { SemVer, SemVerRange } from "./types.ts";
import { testRange } from "./test_range.ts";

/**
 * The minimum valid SemVer for a given range or INVALID
 * @param range The range to calculate the min for
 * @returns A valid SemVer or INVALID
 */
export function rangeMin(range: SemVerRange): SemVer { // For and's, you take the biggest min
  // For or's, you take the smallest min
  //[ [1 and 2] or [2 and 3] ] = [ 2 or 3 ] = 2
  return sort(
    range.ranges.map((r) =>
      sort(r.filter((c) => testRange(c.min, range)).map((c) => c.min)).pop()!
    ).filter((v) => v),
  ).shift() ?? INVALID;
}
