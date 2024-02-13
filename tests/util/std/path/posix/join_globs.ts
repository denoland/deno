// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { GlobOptions } from "../_common/glob_to_reg_exp.ts";
import { join } from "./join.ts";
import { SEP } from "./separator.ts";
import { normalizeGlob } from "./normalize_glob.ts";

/** Like join(), but doesn't collapse "**\/.." when `globstar` is true. */
export function joinGlobs(
  globs: string[],
  { extended = true, globstar = false }: GlobOptions = {},
): string {
  if (!globstar || globs.length === 0) {
    return join(...globs);
  }
  if (globs.length === 0) return ".";
  let joined: string | undefined;
  for (const glob of globs) {
    const path = glob;
    if (path.length > 0) {
      if (!joined) joined = path;
      else joined += `${SEP}${path}`;
    }
  }
  if (!joined) return ".";
  return normalizeGlob(joined, { extended, globstar });
}
