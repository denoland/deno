// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { copyBytes } from "../io/util.ts";

/**
 * Find first index of binary pattern from a. If not found, then return -1
 * @param a soruce array
 * @param b pattern to find in source array
 */
export function findIndex(a: Uint8Array, pat: Uint8Array): number {
  const s = pat[0];
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== s) continue;
    const pin = i;
    let matched = 1;
    let j = i;
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

/**
 * Find last index of binary pattern from a. If not found, then return -1.
 * @param a soruce array
 * @param b pattern to find in source array
 */
export function findLastIndex(a: Uint8Array, pat: Uint8Array): number {
  const e = pat[pat.length - 1];
  for (let i = a.length - 1; i >= 0; i--) {
    if (a[i] !== e) continue;
    const pin = i;
    let matched = 1;
    let j = i;
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

/**
 * Check whether binary arrays are equal to each other.
 * @param a First array to check
 * @param b Second array to check
 */
export function equal(a: Uint8Array, match: Uint8Array): boolean {
  return String(a) === String(match);
}

/**
 * Check whether binary array starts with prefix.
 * @param a First array to concatenate
 * @param b Second array to concatenate
 */
export function hasPrefix(a: Uint8Array, prefix: Uint8Array): boolean {
  return a.length >= prefix.length && equal(a.slice(0, prefix.length), prefix);
}

/**
 * check whether binary array ends with suffix.
 * @param a First Array To Concatenate
 * @param b Second Array To Concatenate
 */
export function hasSuffix(a: Uint8Array, suffix: Uint8Array): boolean {
  return a.length >= suffix.length && equal(a.slice(suffix.length - 1), suffix);
}

/**
 * Repeat bytes. returns a new byte slice consisting of `count` copies of `b`.
 * @param b The origin bytes
 * @param count The count you want to repeat.
 */
export function repeat(b: Uint8Array, count: number): Uint8Array {
  if (count === 0) {
    return new Uint8Array();
  }

  if (count < 0) {
    throw new Error("bytes: negative repeat count");
  } else if ((b.length * count) / count !== b.length) {
    throw new Error("bytes: repeat count causes overflow");
  }

  const int = Math.floor(count);

  if (int !== count) {
    throw new Error("bytes: repeat count must be an integer");
  }

  const nb = new Uint8Array(b.length * count);

  let bp = copyBytes(nb, b);

  for (; bp < nb.length; bp *= 2) {
    copyBytes(nb, nb.slice(0, bp), bp);
  }

  return nb;
}

/**
 * Concatenate two binary arrays and return new one.
 * @param a First array to concatenate
 * @param b Second array to concatenate
 */
export function concat(a: Uint8Array, b: Uint8Array): Uint8Array {
  const output = new Uint8Array(a.length + b.length);
  output.set(a, 0);
  output.set(b, a.length);
  return output;
}

/**
 * Check srouce array contains pattern array.
 * @param s srouce array
 * @param pat patter array
 */
export function contains(s: Uint8Array, pat: Uint8Array): boolean {
  return findIndex(s, pat) != -1;
}
