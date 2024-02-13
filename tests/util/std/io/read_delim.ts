// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { concat } from "../bytes/concat.ts";
import type { Reader } from "../types.d.ts";

/** Generate longest proper prefix which is also suffix array. */
function createLPS(pat: Uint8Array): Uint8Array {
  const lps = new Uint8Array(pat.length);
  lps[0] = 0;
  let prefixEnd = 0;
  let i = 1;
  while (i < lps.length) {
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

/**
 * Read delimited bytes from a Reader.
 *
 * @deprecated (will be removed after 1.0.0) Use the [Web Streams API]{@link https://developer.mozilla.org/en-US/docs/Web/API/Streams_API} instead.
 */
export async function* readDelim(
  reader: Reader,
  delim: Uint8Array,
): AsyncIterableIterator<Uint8Array> {
  // Avoid unicode problems
  const delimLen = delim.length;
  const delimLPS = createLPS(delim);
  let chunks = new Uint8Array();
  const bufSize = Math.max(1024, delimLen + 1);

  // Modified KMP
  let inspectIndex = 0;
  let matchIndex = 0;
  while (true) {
    const inspectArr = new Uint8Array(bufSize);
    const result = await reader.read(inspectArr);
    if (result === null) {
      // Yield last chunk.
      yield chunks;
      return;
    } else if (result < 0) {
      // Discard all remaining and silently fail.
      return;
    }
    chunks = concat([chunks, inspectArr.slice(0, result)]);
    let localIndex = 0;
    while (inspectIndex < chunks.length) {
      if (inspectArr[localIndex] === delim[matchIndex]) {
        inspectIndex++;
        localIndex++;
        matchIndex++;
        if (matchIndex === delimLen) {
          // Full match
          const matchEnd = inspectIndex - delimLen;
          const readyBytes = chunks.slice(0, matchEnd);
          yield readyBytes;
          // Reset match, different from KMP.
          chunks = chunks.slice(inspectIndex);
          inspectIndex = 0;
          matchIndex = 0;
        }
      } else {
        if (matchIndex === 0) {
          inspectIndex++;
          localIndex++;
        } else {
          matchIndex = delimLPS[matchIndex - 1];
        }
      }
    }
  }
}
