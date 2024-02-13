// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { validateBinaryLike } from "./_util.ts";

/**
 * {@linkcode encodeBase58} and {@linkcode decodeBase58} for
 * [base58](https://en.wikipedia.org/wiki/Binary-to-text_encoding#Base58) encoding.
 *
 * This module is browser compatible.
 *
 * @module
 */

// deno-fmt-ignore
const mapBase58: Record<string, number> = {
  "1": 0, "2": 1, "3": 2, "4": 3, "5": 4, "6": 5, "7": 6, "8": 7, "9": 8, A: 9,
  B: 10, C: 11, D: 12, E: 13, F: 14, G: 15, H: 16, J: 17, K: 18, L: 19, M: 20,
  N: 21, P: 22, Q: 23, R: 24, S: 25, T: 26, U: 27, V: 28, W: 29, X: 30, Y: 31,
  Z: 32, a: 33, b: 34, c: 35, d: 36, e: 37, f: 38, g: 39, h: 40, i: 41, j: 42,
  k: 43, m: 44, n: 45, o: 46, p: 47, q: 48, r: 49, s: 50, t: 51, u: 52, v: 53,
  w: 54, x: 55, y: 56, z: 57
};

const base58alphabet =
  "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".split("");

/**
 * @deprecated (will be removed in 0.210.0) Use {@linkcode encodeBase58} instead.
 *
 * Encodes a given Uint8Array, ArrayBuffer or string into draft-mspotny-base58-03 RFC base58 representation:
 * https://tools.ietf.org/id/draft-msporny-base58-01.html#rfc.section.1
 *
 * @param data
 *
 * @returns Encoded value
 */
export const encode = encodeBase58;

/**
 * @deprecated (will be removed in 0.210.0) Use {@linkcode decodeBase58} instead.
 *
 * Decodes a given b58 string according to draft-mspotny-base58-03 RFC base58 representation:
 * https://tools.ietf.org/id/draft-msporny-base58-01.html#rfc.section.1
 *
 * @param b58
 *
 * @returns Decoded value
 */
export const decode = decodeBase58;

/**
 * Encodes a given Uint8Array, ArrayBuffer or string into draft-mspotny-base58-03 RFC base58 representation:
 * https://tools.ietf.org/id/draft-msporny-base58-01.html#rfc.section.1
 */
export function encodeBase58(data: ArrayBuffer | Uint8Array | string): string {
  const uint8tData = validateBinaryLike(data);

  let length = 0;
  let zeroes = 0;

  // Counting leading zeroes
  let index = 0;
  while (uint8tData[index] === 0) {
    zeroes++;
    index++;
  }

  const notZeroUint8Data = uint8tData.slice(index);

  const size = Math.round((uint8tData.length * 138) / 100 + 1);
  const b58Encoding: number[] = [];

  notZeroUint8Data.forEach((byte) => {
    let i = 0;
    let carry = byte;

    for (
      let reverse_iterator = size - 1;
      (carry > 0 || i < length) && reverse_iterator !== -1;
      reverse_iterator--, i++
    ) {
      carry += (b58Encoding[reverse_iterator] || 0) * 256;
      b58Encoding[reverse_iterator] = Math.round(carry % 58);
      carry = Math.floor(carry / 58);
    }

    length = i;
  });

  const strResult: string[] = Array.from({
    length: b58Encoding.length + zeroes,
  });

  if (zeroes > 0) {
    strResult.fill("1", 0, zeroes);
  }

  b58Encoding.forEach((byteValue) => strResult.push(base58alphabet[byteValue]));

  return strResult.join("");
}

/**
 * Decodes a given b58 string according to draft-mspotny-base58-03 RFC base58 representation:
 * https://tools.ietf.org/id/draft-msporny-base58-01.html#rfc.section.1
 */
export function decodeBase58(b58: string): Uint8Array {
  const splitInput = b58.trim().split("");

  let length = 0;
  let ones = 0;

  // Counting leading ones
  let index = 0;
  while (splitInput[index] === "1") {
    ones++;
    index++;
  }

  const notZeroData = splitInput.slice(index);

  const size = Math.round((b58.length * 733) / 1000 + 1);
  const output: number[] = [];

  notZeroData.forEach((char, idx) => {
    let carry = mapBase58[char];
    let i = 0;

    if (carry === undefined) {
      throw new Error(`Invalid base58 char at index ${idx} with value ${char}`);
    }

    for (
      let reverse_iterator = size - 1;
      (carry > 0 || i < length) && reverse_iterator !== -1;
      reverse_iterator--, i++
    ) {
      carry += 58 * (output[reverse_iterator] || 0);
      output[reverse_iterator] = Math.round(carry % 256);
      carry = Math.floor(carry / 256);
    }

    length = i;
  });

  const validOutput = output.filter((item) => item !== undefined);

  if (ones > 0) {
    const onesResult = Array.from({ length: ones }).fill(0, 0, ones);

    return new Uint8Array([...onesResult, ...validOutput] as number[]);
  }

  return new Uint8Array(validOutput);
}
