// Copyright 2018-2026 the Deno authors. MIT license.
(function () {
const { core, primordials } = __bootstrap;
const { Encodings } = core.loadExtScript(
  "ext:deno_node/internal_binding/_node.ts",
);

const {
  Error,
  MathMax,
  MathMin,
  MathTrunc,
  NumberIsNaN,
  TypedArrayPrototypeGetLength,
} = primordials;

function fill(
  buffer,
  value,
  start,
  end,
) {
  // Ignore primordial: `fill` is a method from Node.js Buffer.
  // deno-lint-ignore prefer-primordials
  return buffer.fill(value, start, end);
}

// Mirror of `IndexOfOffset` (node/src/node_buffer.cc). Normalizes a raw
// `offset` (which may be negative or out of bounds) into a start index, or -1
// when there can be no match.
function indexOfOffset(
  length: number,
  offset: number,
  needleLength: number,
  isForward: boolean,
): number {
  if (offset < 0) {
    if (offset + length >= 0) {
      // Negative offsets count backwards from the end of the buffer.
      return length + offset;
    } else if (isForward || needleLength === 0) {
      // From before the start of the buffer: search the whole buffer.
      return 0;
    }
    // lastIndexOf from before the start of the buffer: no match.
    return -1;
  }
  if (offset + needleLength <= length) {
    return offset;
  } else if (needleLength === 0) {
    // Out of bounds, but empty needle: point to end of buffer.
    return length;
  } else if (isForward) {
    // indexOf from past the end of the buffer: no match.
    return -1;
  }
  // lastIndexOf from past the end of the buffer: search the whole buffer.
  return length - 1;
}

// Mirror of `nbytes::SearchString`: searches `source[0, end)` for `needle`
// (`needleLength` bytes) in `step`-byte units (2 for UCS2), returning the byte
// index of the occurrence nearest the requested end of the range, or -1.
function searchBytes(
  source: Uint8Array,
  end: number,
  needle: Uint8Array,
  needleLength: number,
  offset: number,
  isForward: boolean,
  step: number,
): number {
  if (isForward) {
    for (let i = offset; i + needleLength <= end; i += step) {
      let j = 0;
      while (j < needleLength && source[i + j] === needle[j]) j++;
      if (j === needleLength) return i;
    }
    return -1;
  }
  let start = end - needleLength;
  if (offset < start) start = offset;
  start -= start % step; // align to the code-unit grid (UCS2 uses offset/2)
  for (let i = start; i >= 0; i -= step) {
    let j = 0;
    while (j < needleLength && source[i + j] === needle[j]) j++;
    if (j === needleLength) return i;
  }
  return -1;
}

// Mirror of `IndexOfBuffer` (node/src/node_buffer.cc). Handles both string
// needles (pre-encoded to bytes by the encodingOps shims) and Uint8Array
// needles.
function indexOfBuffer(
  targetBuffer: Uint8Array,
  buffer: Uint8Array,
  byteOffset: number,
  encoding: Encodings,
  forwardDirection: boolean,
  end: number,
) {
  if (Encodings[encoding] === undefined) {
    throw new Error(`Unknown encoding code ${encoding}`);
  }
  // `byteOffset` and `end` reach us uncoerced from JS; Node reads each as an
  // int64 at the binding boundary, truncating a fractional value toward zero
  // (and NaN -> 0) before clamping. `byteOffset` is already NaN-resolved by the
  // caller, so it only needs truncation.
  byteOffset = MathTrunc(byteOffset);
  end = NumberIsNaN(end) ? 0 : MathTrunc(end);

  const haystackLength = TypedArrayPrototypeGetLength(targetBuffer);
  const needleLength = TypedArrayPrototypeGetLength(buffer);
  const isUcs2 = encoding === Encodings.UCS2;

  // search_end is the exclusive upper bound of the search range.
  let searchEnd = MathMin(MathMax(end, 0), haystackLength);
  if (isUcs2) searchEnd &= ~1;

  const optOffset = indexOfOffset(
    haystackLength,
    byteOffset,
    needleLength,
    forwardDirection,
  );

  if (needleLength === 0) {
    // Empty needle: match String#indexOf behavior, clamped to search_end.
    return MathMin(optOffset, searchEnd);
  }
  if (haystackLength === 0) return -1;
  if (optOffset <= -1) return -1;

  let offset = optOffset;
  if (!forwardDirection && offset >= searchEnd) {
    if (searchEnd === 0) return -1;
    offset = searchEnd - 1;
  } else if (forwardDirection && offset >= searchEnd) {
    return -1;
  }
  if (
    (forwardDirection && needleLength + offset > searchEnd) ||
    needleLength > searchEnd
  ) {
    return -1;
  }
  if (isUcs2 && (searchEnd < 2 || needleLength < 2)) return -1;

  // For UCS2, Node searches the uint16 view: it aligns `offset` down to the
  // code-unit grid (offset / 2) and drops any trailing odd needle byte
  // (needle_length / 2). Mirror that here in byte space. The guards above still
  // use the raw `needleLength`, matching Node.
  return searchBytes(
    targetBuffer,
    searchEnd,
    buffer,
    isUcs2 ? needleLength & ~1 : needleLength,
    isUcs2 ? offset & ~1 : offset,
    forwardDirection,
    isUcs2 ? 2 : 1,
  );
}

// Mirror of `IndexOfNumberImpl` (node/src/node_buffer.cc).
function indexOfNumber(
  targetBuffer: Uint8Array,
  number: number,
  byteOffset: number,
  forwardDirection: boolean,
  end: number,
) {
  // Node reads `byteOffset`/`end` as int64 at the binding boundary, truncating
  // a fractional value toward zero. `byteOffset` is already NaN-resolved by the
  // caller.
  byteOffset = MathTrunc(byteOffset);
  end = NumberIsNaN(end) ? 0 : MathTrunc(end);
  // Uses only the last byte of the number.
  // https://github.com/nodejs/node/issues/7591#issuecomment-231178104
  number &= 255;
  const bufferLength = TypedArrayPrototypeGetLength(targetBuffer);
  const optOffset = indexOfOffset(
    bufferLength,
    byteOffset,
    1,
    forwardDirection,
  );
  if (optOffset <= -1 || bufferLength === 0) return -1;

  const offset = optOffset;
  const searchEnd = MathMin(MathMax(end, 0), bufferLength);
  if (forwardDirection) {
    if (offset >= searchEnd) return -1;
    for (let i = offset; i < searchEnd; i++) {
      if (targetBuffer[i] === number) return i;
    }
    return -1;
  }
  const backwardEnd = MathMin(offset + 1, searchEnd);
  if (backwardEnd === 0) return -1;
  for (let i = backwardEnd - 1; i >= 0; i--) {
    if (targetBuffer[i] === number) return i;
  }
  return -1;
}

const _defaultExport = { indexOfBuffer, indexOfNumber };

return {
  indexOfBuffer,
  indexOfNumber,
  fill,
  default: _defaultExport,
};
})();
