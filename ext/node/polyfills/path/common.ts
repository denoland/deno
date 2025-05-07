// Copyright 2018-2025 the Deno authors. MIT license.
// This module is browser compatible.

import { primordials } from "ext:core/mod.js";
const {
  StringPrototypeSubstring,
  StringPrototypeLastIndexOf,
  StringPrototypeSplit,
  ArrayPrototypeSlice,
  StringPrototypeEndsWith,
  ArrayPrototypeJoin,
  SafeArrayIterator,
} = primordials;
import { SEP } from "ext:deno_node/path/separator.ts";

/** Determines the common path from a set of paths, using an optional separator,
 * which defaults to the OS default separator.
 *
 * ```ts
 *       const p = common([
 *         "./deno/std/path/mod.ts",
 *         "./deno/std/fs/mod.ts",
 *       ]);
 *       console.log(p); // "./deno/std/"
 * ```
 */
export function common(paths: string[], sep = SEP): string {
  const [first = "", ...remaining] = new SafeArrayIterator(paths);
  if (first === "" || remaining.length === 0) {
    return StringPrototypeSubstring(
      first,
      0,
      StringPrototypeLastIndexOf(first, sep) + 1,
    );
  }
  const parts = StringPrototypeSplit(first, sep);

  let endOfPrefix = parts.length;
  for (const path of new SafeArrayIterator(remaining)) {
    const compare = StringPrototypeSplit(path, sep);
    for (let i = 0; i < endOfPrefix; i++) {
      if (compare[i] !== parts[i]) {
        endOfPrefix = i;
      }
    }

    if (endOfPrefix === 0) {
      return "";
    }
  }
  const prefix = ArrayPrototypeJoin(
    ArrayPrototypeSlice(parts, 0, endOfPrefix),
    sep,
  );
  return StringPrototypeEndsWith(prefix, sep) ? prefix : `${prefix}${sep}`;
}
