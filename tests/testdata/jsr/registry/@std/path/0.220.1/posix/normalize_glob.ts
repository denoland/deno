// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import type { GlobOptions } from "../_common/glob_to_reg_exp.ts";
import { normalize } from "./normalize.ts";
import { SEPARATOR_PATTERN } from "./constants.ts";

export type { GlobOptions };

/** Like normalize(), but doesn't collapse "**\/.." when `globstar` is true. */
export function normalizeGlob(
  glob: string,
  { globstar = false }: GlobOptions = {},
): string {
  if (glob.match(/\0/g)) {
    throw new Error(`Glob contains invalid characters: "${glob}"`);
  }
  if (!globstar) {
    return normalize(glob);
  }
  const s = SEPARATOR_PATTERN.source;
  const badParentPattern = new RegExp(
    `(?<=(${s}|^)\\*\\*${s})\\.\\.(?=${s}|$)`,
    "g",
  );
  return normalize(glob.replace(badParentPattern, "\0")).replace(/\0/g, "..");
}
