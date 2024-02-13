// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { ValueType } from "./encode.ts";

/**
 * Decode a value from the MessagePack binary format.
 *
 * @example
 * ```ts
 * import { decode } from "https://deno.land/std@$STD_VERSION/msgpack/decode.ts";
 *
 * const encoded = Uint8Array.of(1, 2, 3)
 *
 * console.log(decode(encoded))
 * ```
 */
export function decode(uint8: Uint8Array) {
  const pointer = { consumed: 0 };
  const dataView = new DataView(
    uint8.buffer,
    uint8.byteOffset,
    uint8.byteLength,
  );
  const value = decodeSlice(uint8, dataView, pointer);

  if (pointer.consumed < uint8.length) {
    throw new EvalError("Messagepack decode did not consume whole array");
  }

  return value;
}

function decodeString(
  uint8: Uint8Array,
  size: number,
  pointer: { consumed: number },
) {
  pointer.consumed += size;
  return decoder.decode(
    uint8.subarray(pointer.consumed - size, pointer.consumed),
  );
}

function decodeArray(
  uint8: Uint8Array,
  dataView: DataView,
  size: number,
  pointer: { consumed: number },
) {
  const arr: ValueType[] = [];

  for (let i = 0; i < size; i++) {
    const value = decodeSlice(uint8, dataView, pointer);
    arr.push(value);
  }

  return arr;
}

function decodeMap(
  uint8: Uint8Array,
  dataView: DataView,
  size: number,
  pointer: { consumed: number },
) {
  const map: Record<number | string, ValueType> = {};

  for (let i = 0; i < size; i++) {
    const key = decodeSlice(uint8, dataView, pointer);
    const value = decodeSlice(uint8, dataView, pointer);

    if (typeof key !== "number" && typeof key !== "string") {
      throw new EvalError(
        "Messagepack decode came across an invalid type for a key of a map",
      );
    }

    map[key] = value;
  }

  return map;
}

const decoder = new TextDecoder();

const FIXMAP_BITS = 0b1000_0000;
const FIXMAP_MASK = 0b1111_0000;
const FIXARRAY_BITS = 0b1001_0000;
const FIXARRAY_MASK = 0b1111_0000;
const FIXSTR_BITS = 0b1010_0000;
const FIXSTR_MASK = 0b1110_0000;

/**
 * Given a uint8array which contains a msgpack object,
 * return the value of the object as well as how many bytes
 * were consumed in obtaining this object
 */
function decodeSlice(
  uint8: Uint8Array,
  dataView: DataView,
  pointer: { consumed: number },
): ValueType {
  const type = dataView.getUint8(pointer.consumed);
  pointer.consumed++;

  if (type <= 0x7f) { // positive fixint - really small positive number
    return type;
  }

  if ((type & FIXMAP_MASK) === FIXMAP_BITS) { // fixmap - small map
    const size = type & ~FIXMAP_MASK;
    return decodeMap(uint8, dataView, size, pointer);
  }

  if ((type & FIXARRAY_MASK) === FIXARRAY_BITS) { // fixarray - small array
    const size = type & ~FIXARRAY_MASK;
    return decodeArray(uint8, dataView, size, pointer);
  }

  if ((type & FIXSTR_MASK) === FIXSTR_BITS) { // fixstr - small string
    const size = type & ~FIXSTR_MASK;
    return decodeString(uint8, size, pointer);
  }

  if (type >= 0xe0) { // negative fixint - really small negative number
    return type - 256;
  }

  switch (type) {
    case 0xc0: // nil
      return null;
    case 0xc1: // (never used)
      throw new Error(
        "Messagepack decode encountered a type that is never used",
      );
    case 0xc2: // false
      return false;
    case 0xc3: // true
      return true;
    case 0xc4: { // bin 8 - small Uint8Array
      const length = dataView.getUint8(pointer.consumed);
      pointer.consumed++;
      const u8 = uint8.subarray(pointer.consumed, pointer.consumed + length);
      pointer.consumed += length;
      return u8;
    }
    case 0xc5: { // bin 16 - medium Uint8Array
      const length = dataView.getUint16(pointer.consumed);
      pointer.consumed += 2;
      const u8 = uint8.subarray(pointer.consumed, pointer.consumed + length);
      pointer.consumed += length;
      return u8;
    }
    case 0xc6: { // bin 32 - large Uint8Array
      const length = dataView.getUint32(pointer.consumed);
      pointer.consumed += 4;
      const u8 = uint8.subarray(pointer.consumed, pointer.consumed + length);
      pointer.consumed += length;
      return u8;
    }
    case 0xc7: // ext 8 - small extension type
    case 0xc8: // ext 16 - medium extension type
    case 0xc9: // ext 32 - large extension type
      throw new Error("ext not implemented yet");
    case 0xca: { // float 32
      const value = dataView.getFloat32(pointer.consumed);
      pointer.consumed += 4;
      return value;
    }
    case 0xcb: { // float 64
      const value = dataView.getFloat64(pointer.consumed);
      pointer.consumed += 8;
      return value;
    }
    case 0xcc: { // uint 8
      const value = dataView.getUint8(pointer.consumed);
      pointer.consumed += 1;
      return value;
    }
    case 0xcd: { // uint 16
      const value = dataView.getUint16(pointer.consumed);
      pointer.consumed += 2;
      return value;
    }
    case 0xce: { // uint 32
      const value = dataView.getUint32(pointer.consumed);
      pointer.consumed += 4;
      return value;
    }
    case 0xcf: { // uint 64
      const value = dataView.getBigUint64(pointer.consumed);
      pointer.consumed += 8;
      return value;
    }
    case 0xd0: { // int 8
      const value = dataView.getInt8(pointer.consumed);
      pointer.consumed += 1;
      return value;
    }
    case 0xd1: { // int 16
      const value = dataView.getInt16(pointer.consumed);
      pointer.consumed += 2;
      return value;
    }
    case 0xd2: { // int 32
      const value = dataView.getInt32(pointer.consumed);
      pointer.consumed += 4;
      return value;
    }
    case 0xd3: { // int 64
      const value = dataView.getBigInt64(pointer.consumed);
      pointer.consumed += 8;
      return value;
    }
    case 0xd4: // fixext 1 - 1 byte extension type
    case 0xd5: // fixext 2 - 2 byte extension type
    case 0xd6: // fixext 4 - 4 byte extension type
    case 0xd7: // fixext 8 - 8 byte extension type
    case 0xd8: // fixext 16 - 16 byte extension type
      throw new Error("fixext not implemented yet");
    case 0xd9: { // str 8 - small string
      const length = dataView.getUint8(pointer.consumed);
      pointer.consumed += 1;
      return decodeString(uint8, length, pointer);
    }
    case 0xda: { // str 16 - medium string
      const length = dataView.getUint16(pointer.consumed);
      pointer.consumed += 2;
      return decodeString(uint8, length, pointer);
    }
    case 0xdb: { // str 32 - large string
      const length = dataView.getUint32(pointer.consumed);
      pointer.consumed += 4;
      return decodeString(uint8, length, pointer);
    }
    case 0xdc: { // array 16 - medium array
      const length = dataView.getUint16(pointer.consumed);
      pointer.consumed += 2;
      return decodeArray(uint8, dataView, length, pointer);
    }
    case 0xdd: { // array 32 - large array
      const length = dataView.getUint32(pointer.consumed);
      pointer.consumed += 4;
      return decodeArray(uint8, dataView, length, pointer);
    }
    case 0xde: { // map 16 - medium map
      const length = dataView.getUint16(pointer.consumed);
      pointer.consumed += 2;
      return decodeMap(uint8, dataView, length, pointer);
    }
    case 0xdf: { // map 32 - large map
      const length = dataView.getUint32(pointer.consumed);
      pointer.consumed += 4;
      return decodeMap(uint8, dataView, length, pointer);
    }
  }

  // All cases are covered for numbers between 0-255. Typescript isn't smart enough to know that.
  throw new Error("Unreachable");
}
