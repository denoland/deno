// Copyright 2018-2025 the Deno authors. MIT license.
// This module is browser compatible.

import { assertPath } from "../_common/assert_path.ts";
import { normalize } from "./normalize.ts";

/**
 * Join all given a sequence of `paths`,then normalizes the resulting path.
 * @param paths to be joined and normalized
 */
export function join(...paths: string[]): string {
  if (paths.length === 0) return ".";

  let joined: string | undefined;
  for (let i = 0; i < paths.length; ++i) {
    const path = paths[i]!;
    assertPath(path);
    if (path.length > 0) {
      if (!joined) joined = path;
      else joined += `/${path}`;
    }
  }
  if (!joined) return ".";
  return normalize(joined);
}
