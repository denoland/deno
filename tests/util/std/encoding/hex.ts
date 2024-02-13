// Copyright 2009 The Go Authors. All rights reserved.
// https://github.com/golang/go/blob/master/LICENSE
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { validateBinaryLike } from "./_util.ts";

/** Port of the Go
 * [encoding/hex](https://github.com/golang/go/blob/go1.12.5/src/encoding/hex/hex.go)
 * library.
 *
 * This module is browser compatible.
 *
 * @example
 * ```ts
 * import {
 *   decodeHex,
 *   encodeHex,
 * } from "https://deno.land/std@$STD_VERSION/encoding/hex.ts";
 *
 * const binary = new TextEncoder().encode("abc");
 * const encoded = encodeHex(binary);
 * console.log(encoded);
 * // => "616263"
 *
 * console.log(decodeHex(encoded));
 * // => Uint8Array(3) [ 97, 98, 99 ]
 * ```
 *
 * @module
 */

const hexTable = new TextEncoder().encode("0123456789abcdef");
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

function errInvalidByte(byte: number) {
  return new TypeError(`Invalid byte '${String.fromCharCode(byte)}'`);
}

function errLength() {
  return new RangeError("Odd length hex string");
}

/** Converts a hex character into its value. */
function fromHexChar(byte: number): number {
  // '0' <= byte && byte <= '9'
  if (48 <= byte && byte <= 57) return byte - 48;
  // 'a' <= byte && byte <= 'f'
  if (97 <= byte && byte <= 102) return byte - 97 + 10;
  // 'A' <= byte && byte <= 'F'
  if (65 <= byte && byte <= 70) return byte - 65 + 10;

  throw errInvalidByte(byte);
}

/**
 * @deprecated (will be removed in 0.210.0) Use {@linkcode encodeHex} instead.
 *
 * Encodes `src` into `src.length * 2` bytes.
 */
export function encode(src: Uint8Array): Uint8Array {
  const dst = new Uint8Array(src.length * 2);
  for (let i = 0; i < dst.length; i++) {
    const v = src[i];
    dst[i * 2] = hexTable[v >> 4];
    dst[i * 2 + 1] = hexTable[v & 0x0f];
  }
  return dst;
}

/** Encodes the source into hex string. */
export function encodeHex(src: string | Uint8Array | ArrayBuffer): string {
  const u8 = validateBinaryLike(src);

  const dst = new Uint8Array(u8.length * 2);
  for (let i = 0; i < dst.length; i++) {
    const v = u8[i];
    dst[i * 2] = hexTable[v >> 4];
    dst[i * 2 + 1] = hexTable[v & 0x0f];
  }
  return textDecoder.decode(dst);
}

/**
 * @deprecated (will be removed in 0.210.0) Use {@linkcode decodeHex} instead.
 *
 * Decodes `src` into `src.length / 2` bytes.
 * If the input is malformed, an error will be thrown.
 */
export function decode(src: Uint8Array): Uint8Array {
  const dst = new Uint8Array(src.length / 2);
  for (let i = 0; i < dst.length; i++) {
    const a = fromHexChar(src[i * 2]);
    const b = fromHexChar(src[i * 2 + 1]);
    dst[i] = (a << 4) | b;
  }

  if (src.length % 2 === 1) {
    // Check for invalid char before reporting bad length,
    // since the invalid char (if present) is an earlier problem.
    fromHexChar(src[dst.length * 2]);
    throw errLength();
  }

  return dst;
}

/** Decodes the given hex string to Uint8Array.
 * If the input is malformed, an error will be thrown. */
export function decodeHex(src: string): Uint8Array {
  const u8 = textEncoder.encode(src);
  const dst = new Uint8Array(u8.length / 2);
  for (let i = 0; i < dst.length; i++) {
    const a = fromHexChar(u8[i * 2]);
    const b = fromHexChar(u8[i * 2 + 1]);
    dst[i] = (a << 4) | b;
  }

  if (u8.length % 2 === 1) {
    // Check for invalid char before reporting bad length,
    // since the invalid char (if present) is an earlier problem.
    fromHexChar(u8[dst.length * 2]);
    throw errLength();
  }

  return dst;
}
