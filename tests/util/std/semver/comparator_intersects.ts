// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { SemVerComparator } from "./types.ts";
import { gte } from "./gte.ts";
import { lte } from "./lte.ts";
/**
 * Returns true if the range of possible versions intersects with the other comparators set of possible versions
 * @param c0 The left side comparator
 * @param c1 The right side comparator
 * @returns True if any part of the comparators intersect
 */
export function comparatorIntersects(
  c0: SemVerComparator,
  c1: SemVerComparator,
): boolean {
  const l0 = c0.min;
  const l1 = c0.max;
  const r0 = c1.min;
  const r1 = c1.max;

  // We calculate the min and max ranges of both comparators.
  // The minimum min is 0.0.0, the maximum max is ANY.
  //
  // Comparators with equality operators have the same min and max.
  //
  // We then check to see if the min's of either range falls within the span of the other range.
  //
  // A couple of intersection examples:
  // ```
  // l0 ---- l1
  //     r0 ---- r1
  // ```
  // ```
  //     l0 ---- l1
  // r0 ---- r1
  // ```
  // ```
  // l0 ------ l1
  //    r0--r1
  // ```
  // ```
  // l0 - l1
  // r0 - r1
  // ```
  //
  // non-intersection example
  // ```
  // l0 -- l1
  //          r0 -- r1
  // ```
  return (gte(l0, r0) && lte(l0, r1)) || (gte(r0, l0) && lte(r0, l1));
}
