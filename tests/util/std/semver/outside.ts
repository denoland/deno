// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { gt } from "./gt.ts";
import { gte } from "./gte.ts";
import { lte } from "./lte.ts";
import { lt } from "./lt.ts";
import { ALL, ANY } from "./constants.ts";
import type { SemVer, SemVerComparator, SemVerRange } from "./types.ts";
import { testRange } from "./test_range.ts";

/**
 * Returns true if the version is outside the bounds of the range in either the
 * high or low direction. The hilo argument must be either the string '>' or
 * '<'. (This is the function called by {@linkcode gtr} and {@linkcode ltr}.)
 * @param version The version to compare to the range
 * @param range The range of possible versions
 * @param hilo The operator for the comparison or both if undefined.
 * @returns True if the version is outside of the range based on the operator
 */
export function outside(
  version: SemVer,
  range: SemVerRange,
  hilo?: ">" | "<",
): boolean {
  if (!hilo) {
    return outside(version, range, ">") ||
      outside(version, range, "<");
  }

  const [gtfn, ltefn, ltfn, comp, ecomp] = (() => {
    switch (hilo) {
      case ">":
        return [gt, lte, lt, ">", ">="];
      case "<":
        return [lt, gte, gt, "<", "<="];
    }
  })();

  if (testRange(version, range)) {
    return false;
  }

  for (const comparators of range.ranges) {
    let high: SemVerComparator | undefined = undefined;
    let low: SemVerComparator | undefined = undefined;
    for (let comparator of comparators) {
      if (comparator.semver === ANY) {
        comparator = ALL;
      }

      high = high || comparator;
      low = low || comparator;
      if (gtfn(comparator.semver, high.semver)) {
        high = comparator;
      } else if (ltfn(comparator.semver, low.semver)) {
        low = comparator;
      }
    }

    if (!high || !low) return true;

    // If the edge version comparator has a operator then our version
    // isn't outside it
    if (high!.operator === comp || high!.operator === ecomp) {
      return false;
    }

    // If the lowest version comparator has an operator and our version
    // is less than it then it isn't higher than the range
    if (
      (!low!.operator || low!.operator === comp) &&
      ltefn(version, low!.semver)
    ) {
      return false;
    } else if (low!.operator === ecomp && ltfn(version, low!.semver)) {
      return false;
    }
  }
  return true;
}
