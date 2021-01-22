// Ported from Go
// https://github.com/golang/go/blob/go1.12.5/src/encoding/hex/hex.go
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

const hexTable = new TextEncoder().encode("0123456789abcdef");

// fromHexChar converts a hex character into its value.
function fromHexChar(byte: number): number {
  // '0' <= byte && byte <= '9'
  if (48 <= byte && byte <= 57) return byte - 48;
  // 'a' <= byte && byte <= 'f'
  if (97 <= byte && byte <= 102) return byte - 97 + 10;
  // 'A' <= byte && byte <= 'F'
  if (65 <= byte && byte <= 70) return byte - 65 + 10;

  throw new TypeError(
    `received invalid byte: ${
      new TextDecoder().decode(new Uint8Array([byte]))
    }`,
  );
}

/**
 * Encode encodes `src` into `encodedLen(src.length)` bytes.
 * @param src
 */
export function encode(src: Uint8Array): Uint8Array {
  const dst = new Uint8Array(_encodedLen(src.length));
  for (let i = 0; i < dst.length; i++) {
    const v = src[i];
    dst[i * 2] = hexTable[v >> 4];
    dst[i * 2 + 1] = hexTable[v & 0x0f];
  }
  return dst;
}

/**
 * Decode decodes `src` into `decodedLen(src.length)` bytes
 * If the input is malformed an error will be thrown
 * the error.
 * @param src
 */
export function decode(src: Uint8Array): Uint8Array {
  const dst = new Uint8Array(_decodedLen(src.length));
  for (let i = 0; i < dst.length; i++) {
    const a = fromHexChar(src[i * 2]);
    const b = fromHexChar(src[i * 2 + 1]);
    dst[i] = (a << 4) | b;
  }

  if (src.length % 2 == 1) {
    // Check for invalid char before reporting bad length,
    // since the invalid char (if present) is an earlier problem.
    fromHexChar(src[dst.length * 2]);
    throw new TypeError("odd length of hex string");
  }

  return dst;
}

/**
 * DecodedLen returns the length of decoding `x` source bytes.
 * Specifically, it returns `x / 2`.
 * @param x
 */
function _decodedLen(x: number): number {
  return x >>> 1;
}

/**
 * EncodedLen returns the length of an encoding of n source bytes. Specifically,
 * it returns n * 2.
 * @param n
 */
function _encodedLen(n: number): number {
  return n * 2;
}
