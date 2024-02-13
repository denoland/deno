// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { SemVer } from "./types.ts";
import { compare } from "./compare.ts";

/** Less than comparison */
export function lt(
  s0: SemVer,
  s1: SemVer,
): boolean {
  return compare(s0, s1) < 0;
}
