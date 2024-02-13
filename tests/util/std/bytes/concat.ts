// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Concatenate an array of {@linkcode Uint8Array}s.
 *
 * @example
 * ```ts
 * import { concat } from "https://deno.land/std@$STD_VERSION/bytes/concat.ts";
 *
 * const a = new Uint8Array([0, 1, 2]);
 * const b = new Uint8Array([3, 4, 5]);
 * concat([a, b]); // Uint8Array(6) [ 0, 1, 2, 3, 4, 5 ]
 * ```
 */
export function concat(buf: Uint8Array[]): Uint8Array;
/**
 * @deprecated (will be removed in 0.209.0) Pass in an array instead of a
 * spread of arguments.
 *
 * Concatenate an array of {@linkcode Uint8Array}s.
 *
 * @example
 * ```ts
 * import { concat } from "https://deno.land/std@$STD_VERSION/bytes/concat.ts";
 *
 * const a = new Uint8Array([0, 1, 2]);
 * const b = new Uint8Array([3, 4, 5]);
 * concat(a, b); // Uint8Array(6) [ 0, 1, 2, 3, 4, 5 ]
 * ```
 */
export function concat(...buf: Uint8Array[]): Uint8Array;
export function concat(...buf: (Uint8Array | Uint8Array[])[]): Uint8Array {
  /**
   * @todo(iuioiua): Revert to the old implementation upon removal of the
   * spread signatures.
   *
   * @see {@link https://github.com/denoland/deno_std/blob/e6c61ba64d547b60076422bbc1f6ad33184cc10a/bytes/concat.ts}
   */
  // No need to concatenate if there is only one element in array or sub-array
  if (buf.length === 1) {
    if (!Array.isArray(buf[0])) {
      return buf[0];
    } else if (buf[0].length === 1) {
      return buf[0][0];
    }
  }

  let length = 0;
  for (const b of buf) {
    if (Array.isArray(b)) {
      for (const b1 of b) {
        length += b1.length;
      }
    } else {
      length += b.length;
    }
  }

  const output = new Uint8Array(length);
  let index = 0;
  for (const b of buf) {
    if (Array.isArray(b)) {
      for (const b1 of b) {
        output.set(b1, index);
        index += b1.length;
      }
    } else {
      output.set(b, index);
      index += b.length;
    }
  }

  return output;
}
