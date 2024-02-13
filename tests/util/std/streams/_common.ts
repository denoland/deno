// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

export const DEFAULT_CHUNK_SIZE = 16_640;
export const DEFAULT_BUFFER_SIZE = 32 * 1024;

/** Generate longest proper prefix which is also suffix array. */
export function createLPS(pat: Uint8Array): Uint8Array {
  const length = pat.length;
  const lps = new Uint8Array(length);
  lps[0] = 0;
  let prefixEnd = 0;
  let i = 1;
  while (i < length) {
    if (pat[i] === pat[prefixEnd]) {
      prefixEnd++;
      lps[i] = prefixEnd;
      i++;
    } else if (prefixEnd === 0) {
      lps[i] = 0;
      i++;
    } else {
      prefixEnd = lps[prefixEnd - 1];
    }
  }
  return lps;
}
