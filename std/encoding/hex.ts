// Ported from Go
// https://github.com/golang/go/blob/go1.12.5/src/encoding/hex/hex.go
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const hextable = new TextEncoder().encode("0123456789abcdef");

export function errInvalidByte(byte: number): Error {
  return new Error(
    "encoding/hex: invalid byte: " +
      new TextDecoder().decode(new Uint8Array([byte]))
  );
}

export function errLength(): Error {
  return new Error("encoding/hex: odd length hex string");
}

// fromHexChar converts a hex character into its value and a success flag.
function fromHexChar(byte: number): [number, boolean] {
  switch (true) {
    case 48 <= byte && byte <= 57: // '0' <= byte && byte <= '9'
      return [byte - 48, true];
    case 97 <= byte && byte <= 102: // 'a' <= byte && byte <= 'f'
      return [byte - 97 + 10, true];
    case 65 <= byte && byte <= 70: // 'A' <= byte && byte <= 'F'
      return [byte - 65 + 10, true];
  }
  return [0, false];
}

/**
 * EncodedLen returns the length of an encoding of n source bytes. Specifically,
 * it returns n * 2.
 * @param n
 */
export function encodedLen(n: number): number {
  return n * 2;
}

/**
 * Encode encodes `src` into `encodedLen(src.length)` bytes of `dst`.
 * As a convenience, it returns the number of bytes written to `dst`
 * but this value is always `encodedLen(src.length)`.
 * Encode implements hexadecimal encoding.
 * @param dst
 * @param src
 */
export function encode(dst: Uint8Array, src: Uint8Array): number {
  const srcLength = encodedLen(src.length);
  if (dst.length !== srcLength) {
    throw new Error("Out of index.");
  }
  for (let i = 0; i < src.length; i++) {
    const v = src[i];
    dst[i * 2] = hextable[v >> 4];
    dst[i * 2 + 1] = hextable[v & 0x0f];
  }
  return srcLength;
}

/**
 * EncodeToString returns the hexadecimal encoding of `src`.
 * @param src
 */
export function encodeToString(src: Uint8Array): string {
  const dest = new Uint8Array(encodedLen(src.length));
  encode(dest, src);
  return new TextDecoder().decode(dest);
}

/**
 * Decode decodes `src` into `decodedLen(src.length)` bytes
 * returning the actual number of bytes written to `dst`.
 * Decode expects that `src` contains only hexadecimal characters and that `src`
 * has even length.
 * If the input is malformed, Decode returns the number of bytes decoded before
 * the error.
 * @param dst
 * @param src
 */
export function decode(
  dst: Uint8Array,
  src: Uint8Array
): [number, Error | void] {
  let i = 0;
  for (; i < Math.floor(src.length / 2); i++) {
    const [a, aOK] = fromHexChar(src[i * 2]);
    if (!aOK) {
      return [i, errInvalidByte(src[i * 2])];
    }
    const [b, bOK] = fromHexChar(src[i * 2 + 1]);
    if (!bOK) {
      return [i, errInvalidByte(src[i * 2 + 1])];
    }

    dst[i] = (a << 4) | b;
  }

  if (src.length % 2 == 1) {
    // Check for invalid char before reporting bad length,
    // since the invalid char (if present) is an earlier problem.
    const [, ok] = fromHexChar(src[i * 2]);
    if (!ok) {
      return [i, errInvalidByte(src[i * 2])];
    }
    return [i, errLength()];
  }

  return [i, undefined];
}

/**
 * DecodedLen returns the length of a decoding of `x` source bytes.
 * Specifically, it returns `x / 2`.
 * @param x
 */
export function decodedLen(x: number): number {
  return Math.floor(x / 2);
}

/**
 * DecodeString returns the bytes represented by the hexadecimal string `s`.
 * DecodeString expects that src contains only hexadecimal characters and that
 * src has even length.
 * If the input is malformed, DecodeString will throws an error.
 * @param s the `string` need to decode to `Uint8Array`
 */
export function decodeString(s: string): Uint8Array {
  const src = new TextEncoder().encode(s);
  // We can use the source slice itself as the destination
  // because the decode loop increments by one and then the 'seen' byte is not
  // used anymore.
  const [n, err] = decode(src, src);

  if (err) {
    throw err;
  }

  return src.slice(0, n);
}
