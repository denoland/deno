// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { SemVer } from "./types.ts";
import { compare } from "./compare.ts";

/** Greater than or equal to comparison */
export function gte(
  s0: SemVer,
  s1: SemVer,
): boolean {
  return compare(s0, s1) >= 0;
}
