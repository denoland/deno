// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-fmt-ignore
const hexTable = new Uint8Array([
  48,  49,  50,  51, 52, 53,
  54,  55,  56,  57, 97, 98,
  99, 100, 101, 102
]);

/** Encodes `src` into `src.length * 2` bytes. */
export function encode(src: Uint8Array): Uint8Array {
  const dst = new Uint8Array(src.length * 2);
  for (let i = 0; i < dst.length; i++) {
    const v = src[i];
    dst[i * 2] = hexTable[v >> 4];
    dst[i * 2 + 1] = hexTable[v & 0x0f];
  }
  return dst;
}
