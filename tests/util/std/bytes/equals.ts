// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/** Check whether binary arrays are equal to each other using 8-bit comparisons.
 * @private
 * @param a first array to check equality
 * @param b second array to check equality
 */
function equalsNaive(a: Uint8Array, b: Uint8Array): boolean {
  for (let i = 0; i < b.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

/** Check whether binary arrays are equal to each other using 32-bit comparisons.
 * @private
 * @param a first array to check equality
 * @param b second array to check equality
 */
function equals32Bit(a: Uint8Array, b: Uint8Array): boolean {
  const len = a.length;
  const compressible = Math.floor(len / 4);
  const compressedA = new Uint32Array(a.buffer, 0, compressible);
  const compressedB = new Uint32Array(b.buffer, 0, compressible);
  for (let i = compressible * 4; i < len; i++) {
    if (a[i] !== b[i]) return false;
  }
  for (let i = 0; i < compressedA.length; i++) {
    if (compressedA[i] !== compressedB[i]) return false;
  }
  return true;
}

/** Check whether binary arrays are equal to each other.
 * @param a first array to check equality
 * @param b second array to check equality
 */
export function equals(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) {
    return false;
  }
  return a.length < 1000 ? equalsNaive(a, b) : equals32Bit(a, b);
}
