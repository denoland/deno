// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { SemVer, SemVerRange } from "./types.ts";
import { sort } from "./sort.ts";
import { testRange } from "./test_range.ts";

/**
 * Returns the highest version in the list that satisfies the range, or `undefined`
 * if none of them do.
 * @param versions The versions to check.
 * @param range The range of possible versions to compare to.
 * @returns The highest version in versions that satisfies the range.
 */
export function maxSatisfying(
  versions: SemVer[],
  range: SemVerRange,
): SemVer | undefined {
  const satisfying = versions.filter((v) => testRange(v, range));
  const sorted = sort(satisfying);
  return sorted.pop();
}
