// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2020 Keith Cirkel. All rights reserved. MIT license.

/**
 * Functions for encoding typed integers in array buffers.
 *
 * @module
 */

// This implementation is a port of https://deno.land/x/varint@v2.0.0 by @keithamus
// This module is browser compatible.

export const MaxUInt64 = 18446744073709551615n;
export const MaxVarIntLen64 = 10;
export const MaxVarIntLen32 = 5;

const MSB = 0x80;
const REST = 0x7f;
const SHIFT = 7;
const MSBN = 0x80n;
const SHIFTN = 7n;

/**
 * Given a `buf`, starting at `offset` (default: 0), begin decoding bytes as
 * VarInt encoded bytes, for a maximum of 10 bytes (offset + 10). The returned
 * tuple is of the decoded varint 32-bit number, and the new offset with which
 * to continue decoding other data.
 *
 * If a `bigint` in return is undesired, the `decode32` function will return a
 * `number`, but this should only be used in cases where the varint is
 * _assured_ to be 32-bits. If in doubt, use `decode()`.
 *
 * To know how many bytes the VarInt took to encode, simply negate `offset`
 * from the returned new `offset`.
 */
export function decode(buf: Uint8Array, offset = 0): [bigint, number] {
  for (
    let i = offset,
      len = Math.min(buf.length, offset + MaxVarIntLen64),
      shift = 0,
      decoded = 0n;
    i < len;
    i += 1, shift += SHIFT
  ) {
    const byte = buf[i];
    decoded += BigInt((byte & REST) * Math.pow(2, shift));
    if (!(byte & MSB) && decoded > MaxUInt64) {
      throw new RangeError("overflow varint");
    }
    if (!(byte & MSB)) return [decoded, i + 1];
  }
  throw new RangeError("malformed or overflow varint");
}

/**
 * Given a `buf`, starting at `offset` (default: 0), begin decoding bytes as
 * VarInt encoded bytes, for a maximum of 5 bytes (offset + 5). The returned
 * tuple is of the decoded varint 32-bit number, and the new offset with which
 * to continue decoding other data.
 *
 * VarInts are _not 32-bit by default_ so this should only be used in cases
 * where the varint is _assured_ to be 32-bits. If in doubt, use `decode()`.
 *
 * To know how many bytes the VarInt took to encode, simply negate `offset`
 * from the returned new `offset`.
 */
export function decode32(buf: Uint8Array, offset = 0): [number, number] {
  for (
    let i = offset,
      len = Math.min(buf.length, offset + MaxVarIntLen32),
      shift = 0,
      decoded = 0;
    i <= len;
    i += 1, shift += SHIFT
  ) {
    const byte = buf[i];
    decoded += (byte & REST) * Math.pow(2, shift);
    if (!(byte & MSB)) return [decoded, i + 1];
  }
  throw new RangeError("malformed or overflow varint");
}

/**
 * Takes unsigned number `num` and converts it into a VarInt encoded
 * `Uint8Array`, returning a tuple consisting of a `Uint8Array` slice of the
 * encoded VarInt, and an offset where the VarInt encoded bytes end within the
 * `Uint8Array`.
 *
 * If `buf` is not given then a Uint8Array will be created.
 * `offset` defaults to `0`.
 *
 * If passed `buf` then that will be written into, starting at `offset`. The
 * resulting returned `Uint8Array` will be a slice of `buf`. The resulting
 * returned number is effectively `offset + bytesWritten`.
 */
export function encode(
  num: bigint | number,
  buf: Uint8Array = new Uint8Array(MaxVarIntLen64),
  offset = 0,
): [Uint8Array, number] {
  num = BigInt(num);
  if (num < 0n) throw new RangeError("signed input given");
  for (
    let i = offset, len = Math.min(buf.length, MaxVarIntLen64);
    i <= len;
    i += 1
  ) {
    if (num < MSBN) {
      buf[i] = Number(num);
      i += 1;
      return [buf.slice(offset, i), i];
    }
    buf[i] = Number((num & 0xFFn) | MSBN);
    num >>= SHIFTN;
  }
  throw new RangeError(`${num} overflows uint64`);
}
