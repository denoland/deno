// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

/** Find first index of binary pattern from a. If not found, then return -1 **/
export function bytesFindIndex(a: Uint8Array, pat: Uint8Array): number {
  const s = pat[0];
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== s) continue;
    const pin = i;
    let matched = 1,
      j = i;
    while (matched < pat.length) {
      j++;
      if (a[j] !== pat[j - pin]) {
        break;
      }
      matched++;
    }
    if (matched === pat.length) {
      return pin;
    }
  }
  return -1;
}

/** Find last index of binary pattern from a. If not found, then return -1 **/
export function bytesFindLastIndex(a: Uint8Array, pat: Uint8Array): number {
  const e = pat[pat.length - 1];
  for (let i = a.length - 1; i >= 0; i--) {
    if (a[i] !== e) continue;
    const pin = i;
    let matched = 1,
      j = i;
    while (matched < pat.length) {
      j--;
      if (a[j] !== pat[pat.length - 1 - (pin - j)]) {
        break;
      }
      matched++;
    }
    if (matched === pat.length) {
      return pin - pat.length + 1;
    }
  }
  return -1;
}

/** Check whether binary arrays are equal to each other **/
export function bytesEqual(a: Uint8Array, match: Uint8Array): boolean {
  if (a.length !== match.length) return false;
  for (let i = 0; i < match.length; i++) {
    if (a[i] !== match[i]) return false;
  }
  return true;
}

/** Check whether binary array has binary prefix  **/
export function bytesHasPrefix(a: Uint8Array, prefix: Uint8Array): boolean {
  for (let i = 0, max = prefix.length; i < max; i++) {
    if (a[i] !== prefix[i]) return false;
  }
  return true;
}
