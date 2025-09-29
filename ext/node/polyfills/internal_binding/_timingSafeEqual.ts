// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { Buffer } from "node:buffer";
import {
  ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH,
  ERR_INVALID_ARG_TYPE,
} from "ext:deno_node/internal/errors.ts";
import { core } from "ext:core/mod.js";

const {
  isAnyArrayBuffer,
  isArrayBufferView,
} = core;

function validateBuffer(
  buf: unknown,
  name: string,
): asserts buf is ArrayBufferLike | ArrayBufferView {
  if (!isAnyArrayBuffer(buf) && !isArrayBufferView(buf)) {
    throw new ERR_INVALID_ARG_TYPE(
      name,
      ["Buffer", "ArrayBuffer", "TypedArray", "DataView"],
      buf,
    );
  }
}

function toDataView(ab: ArrayBufferLike | ArrayBufferView): DataView {
  if (ArrayBuffer.isView(ab)) {
    return new DataView(ab.buffer, ab.byteOffset, ab.byteLength);
  }
  return new DataView(ab);
}

/** Compare to array buffers or data views in a way that timing based attacks
 * cannot gain information about the platform. */
function stdTimingSafeEqual(
  a: ArrayBufferView | ArrayBufferLike | DataView,
  b: ArrayBufferView | ArrayBufferLike | DataView,
): boolean {
  if (a.byteLength !== b.byteLength) {
    throw new ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH();
  }
  if (!(a instanceof DataView)) {
    a = toDataView(a);
  }
  if (!(b instanceof DataView)) {
    b = toDataView(b);
  }
  const length = a.byteLength;
  let out = 0;
  let i = -1;
  while (++i < length) {
    out |= a.getUint8(i) ^ b.getUint8(i);
  }
  return out === 0;
}

export const timingSafeEqual = (
  buf1: Buffer | DataView | ArrayBuffer,
  buf2: Buffer | DataView | ArrayBuffer,
): boolean => {
  validateBuffer(buf1, "buf1");
  validateBuffer(buf2, "buf2");
  if (buf1 instanceof Buffer) {
    buf1 = new DataView(buf1.buffer, buf1.byteOffset, buf1.byteLength);
  }
  if (buf2 instanceof Buffer) {
    buf2 = new DataView(buf2.buffer, buf2.byteOffset, buf2.byteLength);
  }
  return stdTimingSafeEqual(buf1, buf2);
};
