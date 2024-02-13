// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { SemVer, SemVerRange } from "./types.ts";
import { outside } from "./outside.ts";

/** Checks to see if the version is greater than all possible versions of the range. */
export function gtr(
  version: SemVer,
  range: SemVerRange,
): boolean {
  return outside(version, range, ">");
}
