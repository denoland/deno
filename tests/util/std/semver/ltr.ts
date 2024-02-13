// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { SemVer, SemVerRange } from "./types.ts";
import { outside } from "./outside.ts";

/** Greater than range comparison */
export function ltr(
  version: SemVer,
  range: SemVerRange,
): boolean {
  return outside(version, range, "<");
}
