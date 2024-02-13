// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { compare } from "./compare.ts";
import type { SemVer } from "./types.ts";

/** Returns `true` if they're logically equivalent, even if they're not the exact
 * same version object. */
export function eq(s0: SemVer, s1: SemVer): boolean {
  return compare(s0, s1) === 0;
}
