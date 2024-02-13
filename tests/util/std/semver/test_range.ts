// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { SemVer, SemVerRange } from "./types.ts";
import { gte } from "./gte.ts";
import { lte } from "./lte.ts";

/**
 * Test to see if the version satisfies the range.
 * @param version The version to test
 * @param range The range to check
 * @returns true if the version is in the range
 */
export function testRange(version: SemVer, range: SemVerRange): boolean {
  for (const r of range.ranges) {
    if (r.every((c) => gte(version, c.min) && lte(version, c.max))) {
      return true;
    }
  }
  return false;
}
