// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { Buffer } from "node:buffer";

function assert(cond) {
  if (!cond) {
    throw new Error("assertion failed");
  }
}

/** Compare to array buffers or data views in a way that timing based attacks
 * cannot gain information about the platform. */
function stdTimingSafeEqual(
  a: ArrayBufferView | ArrayBufferLike | DataView,
  b: ArrayBufferView | ArrayBufferLike | DataView,
): boolean {
  if (a.byteLength !== b.byteLength) {
    return false;
  }
  if (!(a instanceof DataView)) {
    a = new DataView(ArrayBuffer.isView(a) ? a.buffer : a);
  }
  if (!(b instanceof DataView)) {
    b = new DataView(ArrayBuffer.isView(b) ? b.buffer : b);
  }
  assert(a instanceof DataView);
  assert(b instanceof DataView);
  const length = a.byteLength;
  let out = 0;
  let i = -1;
  while (++i < length) {
    out |= a.getUint8(i) ^ b.getUint8(i);
  }
  return out === 0;
}

export const timingSafeEqual = (
  a: Buffer | DataView | ArrayBuffer,
  b: Buffer | DataView | ArrayBuffer,
): boolean => {
  if (a instanceof Buffer) a = new DataView(a.buffer);
  if (a instanceof Buffer) b = new DataView(a.buffer);
  return stdTimingSafeEqual(a, b);
};
