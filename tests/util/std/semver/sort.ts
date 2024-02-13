// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { SemVer } from "./types.ts";
import { compare } from "./compare.ts";

/** Sorts a list of semantic versions in ascending order. */
export function sort(
  list: SemVer[],
): SemVer[] {
  return list.sort((a, b) => compare(a, b));
}
