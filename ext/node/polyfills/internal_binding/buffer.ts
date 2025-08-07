// Copyright 2018-2025 the Deno authors. MIT license.

import { Encodings } from "ext:deno_node/internal_binding/_node.ts";
import { primordials } from "ext:core/mod.js";

const {
  Error,
  MathMax,
  TypedArrayFrom,
  TypedArrayPrototypeGetLength,
  TypedArrayPrototypeSlice,
  Uint8Array,
} = primordials;

export function fill(
  buffer,
  value,
  start,
  end,
) {
  // Ignore primordial: `fill` is a method from Node.js Buffer.
  // deno-lint-ignore prefer-primordials
  return buffer.fill(value, start, end);
}

export function indexOfNeedle(
  source: Uint8Array,
  needle: Uint8Array,
  start = 0,
  step = 1,
): number {
  const sourceLength = TypedArrayPrototypeGetLength(source);
  const needleLength = TypedArrayPrototypeGetLength(needle);

  if (start >= sourceLength) {
    return -1;
  }
  if (start < 0) {
    start = MathMax(0, sourceLength + start);
  }
  const s = needle[0];
  for (let i = start; i < sourceLength; i += step) {
    if (source[i] !== s) continue;
    const pin = i;
    let matched = 1;
    let j = i;
    while (matched < needleLength) {
      j++;
      if (source[j] !== needle[j - pin]) {
        break;
      }
      matched++;
    }
    if (matched === needleLength) {
      return pin;
    }
  }
  return -1;
}

// TODO(Soremwar)
// Check if offset or buffer can be transform in order to just use std's lastIndexOf directly
// This implementation differs from std's lastIndexOf in the fact that
// it also includes items outside of the offset as long as part of the
// set is contained inside of the offset
// Probably way slower too
function findLastIndex(
  targetBuffer: Uint8Array,
  buffer: Uint8Array,
  offset: number,
) {
  const targetBufferLength = TypedArrayPrototypeGetLength(targetBuffer);
  const bufferLength = TypedArrayPrototypeGetLength(buffer);

  offset = offset > targetBufferLength ? targetBufferLength : offset;

  const searchableBuffer = TypedArrayPrototypeSlice(
    targetBuffer,
    0,
    offset + bufferLength,
  );
  const searchableBufferLastIndex =
    TypedArrayPrototypeGetLength(searchableBuffer) - 1;
  const bufferLastIndex = bufferLength - 1;

  // Important to keep track of the last match index in order to backtrack after an incomplete match
  // Not doing this will cause the search to skip all possible matches that happened in the
  // last match range
  let lastMatchIndex = -1;
  let matches = 0;
  let index = -1;
  for (let x = 0; x <= searchableBufferLastIndex; x++) {
    if (
      searchableBuffer[searchableBufferLastIndex - x] ===
        buffer[bufferLastIndex - matches]
    ) {
      if (lastMatchIndex === -1) {
        lastMatchIndex = x;
      }
      matches++;
    } else {
      matches = 0;
      if (lastMatchIndex !== -1) {
        // Restart the search right after the last index was ignored
        x = lastMatchIndex + 1;
        lastMatchIndex = -1;
      }
      continue;
    }

    if (matches === bufferLength) {
      index = x;
      break;
    }
  }

  if (index === -1) return index;

  return searchableBufferLastIndex - index;
}

function indexOfBuffer(
  targetBuffer: Uint8Array,
  buffer: Uint8Array,
  byteOffset: number,
  encoding: Encodings,
  forwardDirection: boolean,
) {
  if (!Encodings[encoding] === undefined) {
    throw new Error(`Unknown encoding code ${encoding}`);
  }

  const targetBufferLength = TypedArrayPrototypeGetLength(targetBuffer);
  const bufferLength = TypedArrayPrototypeGetLength(buffer);
  const isUcs2 = encoding === Encodings.UCS2;

  // If the encoding is UCS2 and haystack or needle has a length less than 2, the search will always fail
  // https://github.com/nodejs/node/blob/fbdfe9399cf6c660e67fd7d6ceabfb106e32d787/src/node_buffer.cc#L1067-L1069
  if (isUcs2) {
    if (bufferLength < 2 || targetBufferLength < 2) {
      return -1;
    }
  }

  if (!forwardDirection) {
    // If negative the offset is calculated from the end of the buffer

    if (byteOffset < 0) {
      byteOffset = targetBufferLength + byteOffset;
    }

    if (bufferLength === 0) {
      return byteOffset <= targetBufferLength ? byteOffset : targetBufferLength;
    }

    return findLastIndex(targetBuffer, buffer, byteOffset);
  }

  if (buffer.length === 0) {
    return byteOffset <= targetBufferLength ? byteOffset : targetBufferLength;
  }

  return indexOfNeedle(targetBuffer, buffer, byteOffset, isUcs2 ? 2 : 1);
}

function indexOfNumber(
  targetBuffer: Uint8Array,
  number: number,
  byteOffset: number,
  forwardDirection: boolean,
) {
  return indexOfBuffer(
    targetBuffer,
    // Uses only the last 2 hex digits of the number
    // https://github.com/nodejs/node/issues/7591#issuecomment-231178104
    TypedArrayFrom(Uint8Array, [number & 255]),
    byteOffset,
    Encodings.UTF8,
    forwardDirection,
  );
}

export default { indexOfBuffer, indexOfNumber };
export { indexOfBuffer, indexOfNumber };
