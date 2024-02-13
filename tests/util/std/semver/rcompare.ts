// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { SemVer } from "./types.ts";
import { compare } from "./compare.ts";

/**
 * A reverse comparison of two versions. Same as compare but
 * `1` and `-1` are inverted.
 *
 * Sorts in descending order if passed to `Array.sort()`,
 */
export function rcompare(
  s0: SemVer,
  s1: SemVer,
): 1 | 0 | -1 {
  return compare(s1, s0);
}
