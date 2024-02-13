// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.
import { copy } from "./copy.ts";

/** Returns a new Uint8Array composed of `count` repetitions of the `source`
 * array.
 *
 * If `count` is negative, a `RangeError` is thrown.
 *
 * ```ts
 * import { repeat } from "https://deno.land/std@$STD_VERSION/bytes/repeat.ts";
 * const source = new Uint8Array([0, 1, 2]);
 * console.log(repeat(source, 3)); // [0, 1, 2, 0, 1, 2, 0, 1, 2]
 * console.log(repeat(source, 0)); // []
 * console.log(repeat(source, -1)); // RangeError
 * ```
 */
export function repeat(source: Uint8Array, count: number): Uint8Array {
  if (count === 0) {
    return new Uint8Array();
  }

  if (count < 0) {
    throw new RangeError("bytes: negative repeat count");
  }

  if (!Number.isInteger(count)) {
    throw new Error("bytes: repeat count must be an integer");
  }

  const nb = new Uint8Array(source.length * count);

  let bp = copy(source, nb);

  for (; bp < nb.length; bp *= 2) {
    copy(nb.slice(0, bp), nb, bp);
  }

  return nb;
}
