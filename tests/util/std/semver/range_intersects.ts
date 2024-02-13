// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { comparatorIntersects } from "./comparator_intersects.ts";
import type { SemVerComparator, SemVerRange } from "./types.ts";

function rangesSatisfiable(ranges: SemVerRange[]): boolean {
  return ranges.every((r) => {
    // For each OR at least one AND must be satisfiable
    return r.ranges.some((comparators) => comparatorsSatisfiable(comparators));
  });
}

function comparatorsSatisfiable(comparators: SemVerComparator[]): boolean {
  // Comparators are satisfiable if they all intersect with each other
  for (let i = 0; i < comparators.length - 1; i++) {
    const c0 = comparators[i];
    for (const c1 of comparators.slice(i + 1)) {
      if (!comparatorIntersects(c0, c1)) {
        return false;
      }
    }
  }
  return true;
}

/**
 * The ranges intersect every range of AND comparators intersects with a least one range of OR ranges.
 * @param r0 range 0
 * @param r1 range 1
 * @returns returns true if any
 */
export function rangeIntersects(r0: SemVerRange, r1: SemVerRange): boolean {
  return rangesSatisfiable([r0, r1]) && r0.ranges.some((r00) => {
    return r1.ranges.some((r11) => {
      return r00.every((c0) => {
        return r11.every((c1) => comparatorIntersects(c0, c1));
      });
    });
  });
}
