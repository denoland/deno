// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const {
  ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH,
  ERR_INVALID_ARG_TYPE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");

const {
  isAnyArrayBuffer,
  isArrayBufferView,
  isDataView,
} = core;
const {
  ArrayBufferIsView,
  ArrayBufferPrototypeGetByteLength,
  DataView,
  DataViewPrototypeGetBuffer,
  DataViewPrototypeGetByteLength,
  DataViewPrototypeGetByteOffset,
  DataViewPrototypeGetUint8,
  ObjectPrototypeIsPrototypeOf,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
} = primordials;

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

function byteLengthOf(
  ab: ArrayBufferView | ArrayBufferLike | DataView,
): number {
  if (isDataView(ab)) {
    return DataViewPrototypeGetByteLength(ab);
  }
  if (ArrayBufferIsView(ab)) {
    return TypedArrayPrototypeGetByteLength(ab);
  }
  return ArrayBufferPrototypeGetByteLength(ab);
}

function toDataView(ab: ArrayBufferLike | ArrayBufferView): DataView {
  if (ArrayBufferIsView(ab)) {
    if (isDataView(ab)) {
      return new DataView(
        DataViewPrototypeGetBuffer(ab),
        DataViewPrototypeGetByteOffset(ab),
        DataViewPrototypeGetByteLength(ab),
      );
    }
    return new DataView(
      TypedArrayPrototypeGetBuffer(ab),
      TypedArrayPrototypeGetByteOffset(ab),
      TypedArrayPrototypeGetByteLength(ab),
    );
  }
  return new DataView(ab);
}

/** Compare to array buffers or data views in a way that timing based attacks
 * cannot gain information about the platform. */
function stdTimingSafeEqual(
  a: ArrayBufferView | ArrayBufferLike | DataView,
  b: ArrayBufferView | ArrayBufferLike | DataView,
): boolean {
  if (byteLengthOf(a) !== byteLengthOf(b)) {
    throw new ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH();
  }
  if (!isDataView(a)) {
    a = toDataView(a);
  }
  if (!isDataView(b)) {
    b = toDataView(b);
  }
  const length = DataViewPrototypeGetByteLength(a);
  let out = 0;
  let i = -1;
  while (++i < length) {
    out |= DataViewPrototypeGetUint8(a, i) ^ DataViewPrototypeGetUint8(b, i);
  }
  return out === 0;
}

const timingSafeEqual = (
  buf1: Buffer | DataView | ArrayBuffer,
  buf2: Buffer | DataView | ArrayBuffer,
): boolean => {
  validateBuffer(buf1, "buf1");
  validateBuffer(buf2, "buf2");
  if (ObjectPrototypeIsPrototypeOf(Buffer.prototype, buf1)) {
    buf1 = new DataView(
      TypedArrayPrototypeGetBuffer(buf1),
      TypedArrayPrototypeGetByteOffset(buf1),
      TypedArrayPrototypeGetByteLength(buf1),
    );
  }
  if (ObjectPrototypeIsPrototypeOf(Buffer.prototype, buf2)) {
    buf2 = new DataView(
      TypedArrayPrototypeGetBuffer(buf2),
      TypedArrayPrototypeGetByteOffset(buf2),
      TypedArrayPrototypeGetByteLength(buf2),
    );
  }
  return stdTimingSafeEqual(buf1, buf2);
};

return { timingSafeEqual, default: { timingSafeEqual } };
})();
