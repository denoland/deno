// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { core } from "ext:core/mod.js";
import { op_node_fill_random, op_node_fill_random_async } from "ext:core/ops";

import { Buffer, kMaxLength } from "node:buffer";
import { isAnyArrayBuffer, isArrayBufferView } from "node:util/types";
const {
  ERR_INVALID_ARG_TYPE,
  ERR_OUT_OF_RANGE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  validateFunction,
  validateNumber,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");

const kMaxInt32 = 2 ** 31 - 1;
const kMaxPossibleLength = Math.min(kMaxLength, kMaxInt32);

// Mirrors Node's lib/internal/crypto/random.js assertOffset().
function assertOffset(offset, elementSize, length) {
  validateNumber(offset, "offset");
  offset *= elementSize;

  const maxLength = Math.min(length, kMaxPossibleLength);
  if (Number.isNaN(offset) || offset > maxLength || offset < 0) {
    throw new ERR_OUT_OF_RANGE("offset", `>= 0 && <= ${maxLength}`, offset);
  }

  return offset >>> 0;
}

// Mirrors Node's lib/internal/crypto/random.js assertSize().
function assertSize(size, elementSize, offset, length) {
  validateNumber(size, "size");
  size *= elementSize;

  if (Number.isNaN(size) || size > kMaxPossibleLength || size < 0) {
    throw new ERR_OUT_OF_RANGE(
      "size",
      `>= 0 && <= ${kMaxPossibleLength}`,
      size,
    );
  }

  if (size + offset > length) {
    throw new ERR_OUT_OF_RANGE(
      "size + offset",
      `<= ${length}`,
      size + offset,
    );
  }

  return size >>> 0;
}

export default function randomFill(buf, offset, size, cb) {
  if (!isAnyArrayBuffer(buf) && !isArrayBufferView(buf)) {
    throw new ERR_INVALID_ARG_TYPE(
      "buf",
      ["ArrayBuffer", "ArrayBufferView"],
      buf,
    );
  }

  const elementSize = buf.BYTES_PER_ELEMENT || 1;

  if (typeof offset === "function") {
    cb = offset;
    offset = 0;
    // Size is a length here; assertSize() turns it into a number of bytes.
    size = buf.length;
  } else if (typeof size === "function") {
    cb = size;
    size = buf.length - offset;
  } else {
    validateFunction(cb, "callback");
  }

  offset = assertOffset(offset, elementSize, buf.byteLength);

  if (size === undefined) {
    size = buf.byteLength - offset;
  } else {
    size = assertSize(size, elementSize, offset, buf.byteLength);
  }

  if (size === 0) {
    cb(null, buf);
    return;
  }

  op_node_fill_random_async(size).then((randomData) => {
    const randomBuf = Buffer.from(randomData.buffer);
    const target = isAnyArrayBuffer(buf)
      ? new Uint8Array(buf, offset, size)
      : new Uint8Array(
        buf.buffer,
        buf.byteOffset + offset,
        size,
      );
    target.set(new Uint8Array(randomBuf.buffer, 0, size));
    cb(null, buf);
  });
}

export function randomFillSync(buf, offset = 0, size) {
  if (!isAnyArrayBuffer(buf) && !isArrayBufferView(buf)) {
    throw new ERR_INVALID_ARG_TYPE(
      "buf",
      ["ArrayBuffer", "ArrayBufferView"],
      buf,
    );
  }

  const elementSize = buf.BYTES_PER_ELEMENT || 1;

  offset = assertOffset(offset, elementSize, buf.byteLength);

  if (size === undefined) {
    size = buf.byteLength - offset;
  } else {
    size = assertSize(size, elementSize, offset, buf.byteLength);
  }

  if (size === 0) {
    return buf;
  }

  const bytes = isAnyArrayBuffer(buf)
    ? new Uint8Array(buf, offset, size)
    : new Uint8Array(buf.buffer, buf.byteOffset + offset, size);
  op_node_fill_random(bytes);

  return buf;
}
