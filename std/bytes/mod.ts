// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/** Find first index of binary pattern from a. If not found, then return -1
 * @param source source array
 * @param pat pattern to find in source array
 */
export function indexOf(source: Uint8Array, pat: Uint8Array): number {
  const s = pat[0];
  for (let i = 0; i < source.length; i++) {
    if (source[i] !== s) continue;
    const pin = i;
    let matched = 1;
    let j = i;
    while (matched < pat.length) {
      j++;
      if (source[j] !== pat[j - pin]) {
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

/** Find last index of binary pattern from a. If not found, then return -1.
 * @param source source array
 * @param pat pattern to find in source array
 */
export function lastIndexOf(source: Uint8Array, pat: Uint8Array): number {
  const e = pat[pat.length - 1];
  for (let i = source.length - 1; i >= 0; i--) {
    if (source[i] !== e) continue;
    const pin = i;
    let matched = 1;
    let j = i;
    while (matched < pat.length) {
      j--;
      if (source[j] !== pat[pat.length - 1 - (pin - j)]) {
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

/** Check whether binary arrays are equal to each other.
 * @param source first array to check equality
 * @param match second array to check equality
 */
export function equals(source: Uint8Array, match: Uint8Array): boolean {
  if (source.length !== match.length) return false;
  for (let i = 0; i < match.length; i++) {
    if (source[i] !== match[i]) return false;
  }
  return true;
}

/** Check whether binary array starts with prefix.
 * @param source source array
 * @param prefix prefix array to check in source
 */
export function startsWith(source: Uint8Array, prefix: Uint8Array): boolean {
  for (let i = 0, max = prefix.length; i < max; i++) {
    if (source[i] !== prefix[i]) return false;
  }
  return true;
}

/** Check whether binary array ends with suffix.
 * @param source source array
 * @param suffix suffix array to check in source
 */
export function endsWith(source: Uint8Array, suffix: Uint8Array): boolean {
  for (
    let srci = source.length - 1, sfxi = suffix.length - 1;
    sfxi >= 0;
    srci--, sfxi--
  ) {
    if (source[srci] !== suffix[sfxi]) return false;
  }
  return true;
}

/** Repeat bytes. returns a new byte slice consisting of `count` copies of `b`.
 * @param origin The origin bytes
 * @param count The count you want to repeat.
 */
export function repeat(origin: Uint8Array, count: number): Uint8Array {
  if (count === 0) {
    return new Uint8Array();
  }

  if (count < 0) {
    throw new RangeError("bytes: negative repeat count");
  } else if ((origin.length * count) / count !== origin.length) {
    throw new RangeError("bytes: repeat count causes overflow");
  }

  const int = Math.floor(count);

  if (int !== count) {
    throw new TypeError("bytes: repeat count must be an integer");
  }

  const nb = new Uint8Array(origin.length * count);

  let bp = copy(origin, nb);

  for (; bp < nb.length; bp *= 2) {
    copy(nb.slice(0, bp), nb, bp);
  }

  return nb;
}

/** Concatenate multiple binary arrays and return new one.
 * @param origin binary array to concatenate
 * @param buf binary arrays to concatenate with origin
 */
export function concat(a: Uint8Array, ...buf: Uint8Array[]): Uint8Array {
  let totalLength = a.length;
  for (const b of buf) {
    totalLength += b.length;
  }

  const output = new Uint8Array(totalLength);
  output.set(a, 0);

  let lastIndex = a.length;
  for (const b of buf) {
    output.set(b, lastIndex);
    lastIndex += b.length;
  }

  return output;
}

/** Check source array contains pattern array.
 * @param source source array
 * @param pat patter array
 */
export function contains(source: Uint8Array, pat: Uint8Array): boolean {
  return indexOf(source, pat) != -1;
}

/**
 * Copy bytes from one Uint8Array to another.  Bytes from `src` which don't fit
 * into `dst` will not be copied.
 *
 * @param src Source byte array
 * @param dst Destination byte array
 * @param off Offset into `dst` at which to begin writing values from `src`.
 * @return number of bytes copied
 */
export function copy(src: Uint8Array, dst: Uint8Array, off = 0): number {
  off = Math.max(0, Math.min(off, dst.byteLength));
  const dstBytesAvailable = dst.byteLength - off;
  if (src.byteLength > dstBytesAvailable) {
    src = src.subarray(0, dstBytesAvailable);
  }
  dst.set(src, off);
  return src.byteLength;
}
