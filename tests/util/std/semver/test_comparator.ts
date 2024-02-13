// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { SemVer, SemVerComparator } from "./types.ts";
import { cmp } from "./cmp.ts";

/**
 * Test to see if a semantic version falls within the range of the comparator.
 * @param version The version to compare
 * @param comparator The comparator
 * @returns True if the version is within the comparators set otherwise false
 */
export function testComparator(
  version: SemVer,
  comparator: SemVerComparator,
): boolean {
  return cmp(version, comparator.operator, comparator.semver);
}
