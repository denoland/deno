// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { concat } from "../bytes/concat.ts";

export type ValueType =
  | number
  | bigint
  | string
  | boolean
  | null
  | Uint8Array
  | ValueType[]
  | ValueMap;

interface ValueMap {
  [index: string | number]: ValueType;
}

const FOUR_BITS = 16;
const FIVE_BITS = 32;
const SEVEN_BITS = 128;
const EIGHT_BITS = 256;
const FIFTEEN_BITS = 32768;
const SIXTEEN_BITS = 65536;
const THIRTY_ONE_BITS = 2147483648;
const THIRTY_TWO_BITS = 4294967296;
const SIXTY_THREE_BITS = 9223372036854775808n;
const SIXTY_FOUR_BITS = 18446744073709551616n;

const encoder = new TextEncoder();

/**
 * Encode a value to MessagePack binary format.
 *
 * @example
 * ```ts
 * import { encode } from "https://deno.land/std@$STD_VERSION/msgpack/encode.ts";
 *
 * const obj = {
 *   str: "deno",
 *   arr: [1, 2, 3],
 *   map: {
 *     foo: "bar"
 *   }
 * }
 *
 * console.log(encode(obj))
 * ```
 */
export function encode(object: ValueType) {
  const byteParts: Uint8Array[] = [];
  encodeSlice(object, byteParts);
  return concat(byteParts);
}

function encodeFloat64(num: number) {
  const dataView = new DataView(new ArrayBuffer(9));
  dataView.setFloat64(1, num);
  dataView.setUint8(0, 0xcb);
  return new Uint8Array(dataView.buffer);
}

function encodeNumber(num: number) {
  if (!Number.isInteger(num)) { // float 64
    return encodeFloat64(num);
  }

  if (num < 0) {
    if (num >= -FIVE_BITS) { // negative fixint
      return new Uint8Array([num]);
    }

    if (num >= -SEVEN_BITS) { // int 8
      return new Uint8Array([0xd0, num]);
    }

    if (num >= -FIFTEEN_BITS) { // int 16
      const dataView = new DataView(new ArrayBuffer(3));
      dataView.setInt16(1, num);
      dataView.setUint8(0, 0xd1);
      return new Uint8Array(dataView.buffer);
    }

    if (num >= -THIRTY_ONE_BITS) { // int 32
      const dataView = new DataView(new ArrayBuffer(5));
      dataView.setInt32(1, num);
      dataView.setUint8(0, 0xd2);
      return new Uint8Array(dataView.buffer);
    }

    // float 64
    return encodeFloat64(num);
  }

  // if the number fits within a positive fixint, use it
  if (num <= 0x7f) {
    return new Uint8Array([num]);
  }

  if (num < EIGHT_BITS) { // uint8
    return new Uint8Array([0xcc, num]);
  }

  if (num < SIXTEEN_BITS) { // uint16
    const dataView = new DataView(new ArrayBuffer(3));
    dataView.setUint16(1, num);
    dataView.setUint8(0, 0xcd);
    return new Uint8Array(dataView.buffer);
  }

  if (num < THIRTY_TWO_BITS) { // uint32
    const dataView = new DataView(new ArrayBuffer(5));
    dataView.setUint32(1, num);
    dataView.setUint8(0, 0xce);
    return new Uint8Array(dataView.buffer);
  }

  // float 64
  return encodeFloat64(num);
}

function encodeSlice(object: ValueType, byteParts: Uint8Array[]) {
  if (object === null) {
    byteParts.push(new Uint8Array([0xc0]));
    return;
  }

  if (object === false) {
    byteParts.push(new Uint8Array([0xc2]));
    return;
  }

  if (object === true) {
    byteParts.push(new Uint8Array([0xc3]));
    return;
  }

  if (typeof object === "number") {
    byteParts.push(encodeNumber(object));
    return;
  }

  if (typeof object === "bigint") {
    if (object < 0) {
      if (object < -SIXTY_THREE_BITS) {
        throw new Error("Cannot safely encode bigint larger than 64 bits");
      }

      const dataView = new DataView(new ArrayBuffer(9));
      dataView.setBigInt64(1, object);
      dataView.setUint8(0, 0xd3);
      byteParts.push(new Uint8Array(dataView.buffer));
      return;
    }

    if (object >= SIXTY_FOUR_BITS) {
      throw new Error("Cannot safely encode bigint larger than 64 bits");
    }

    const dataView = new DataView(new ArrayBuffer(9));
    dataView.setBigUint64(1, object);
    dataView.setUint8(0, 0xcf);
    byteParts.push(new Uint8Array(dataView.buffer));
    return;
  }

  if (typeof object === "string") {
    const encoded = encoder.encode(object);
    const len = encoded.length;

    if (len < FIVE_BITS) { // fixstr
      byteParts.push(new Uint8Array([0xa0 | len]));
    } else if (len < EIGHT_BITS) { // str 8
      byteParts.push(new Uint8Array([0xd9, len]));
    } else if (len < SIXTEEN_BITS) { // str 16
      const dataView = new DataView(new ArrayBuffer(3));
      dataView.setUint16(1, len);
      dataView.setUint8(0, 0xda);
      byteParts.push(new Uint8Array(dataView.buffer));
    } else if (len < THIRTY_TWO_BITS) { // str 32
      const dataView = new DataView(new ArrayBuffer(5));
      dataView.setUint32(1, len);
      dataView.setUint8(0, 0xdb);
      byteParts.push(new Uint8Array(dataView.buffer));
    } else {
      throw new Error(
        "Cannot safely encode string with size larger than 32 bits",
      );
    }
    byteParts.push(encoded);
    return;
  }

  if (object instanceof Uint8Array) {
    if (object.length < EIGHT_BITS) { // bin 8
      byteParts.push(new Uint8Array([0xc4, object.length]));
    } else if (object.length < SIXTEEN_BITS) { // bin 16
      const dataView = new DataView(new ArrayBuffer(3));
      dataView.setUint16(1, object.length);
      dataView.setUint8(0, 0xc5);
      byteParts.push(new Uint8Array(dataView.buffer));
    } else if (object.length < THIRTY_TWO_BITS) { // bin 32
      const dataView = new DataView(new ArrayBuffer(5));
      dataView.setUint32(1, object.length);
      dataView.setUint8(0, 0xc6);
      byteParts.push(new Uint8Array(dataView.buffer));
    } else {
      throw new Error(
        "Cannot safely encode Uint8Array with size larger than 32 bits",
      );
    }
    byteParts.push(object);
    return;
  }

  if (Array.isArray(object)) {
    if (object.length < FOUR_BITS) { // fixarray
      byteParts.push(new Uint8Array([0x90 | object.length]));
    } else if (object.length < SIXTEEN_BITS) { // array 16
      const dataView = new DataView(new ArrayBuffer(3));
      dataView.setUint16(1, object.length);
      dataView.setUint8(0, 0xdc);
      byteParts.push(new Uint8Array(dataView.buffer));
    } else if (object.length < THIRTY_TWO_BITS) { // array 32
      const dataView = new DataView(new ArrayBuffer(5));
      dataView.setUint32(1, object.length);
      dataView.setUint8(0, 0xdd);
      byteParts.push(new Uint8Array(dataView.buffer));
    } else {
      throw new Error(
        "Cannot safely encode array with size larger than 32 bits",
      );
    }

    for (const obj of object) {
      encodeSlice(obj, byteParts);
    }
    return;
  }

  // If object is a plain object
  if (Object.getPrototypeOf(object) === Object.prototype) {
    const numKeys = Object.keys(object).length;

    if (numKeys < FOUR_BITS) { // fixarray
      byteParts.push(new Uint8Array([0x80 | numKeys]));
    } else if (numKeys < SIXTEEN_BITS) { // map 16
      const dataView = new DataView(new ArrayBuffer(3));
      dataView.setUint16(1, numKeys);
      dataView.setUint8(0, 0xde);
      byteParts.push(new Uint8Array(dataView.buffer));
    } else if (numKeys < THIRTY_TWO_BITS) { // map 32
      const dataView = new DataView(new ArrayBuffer(5));
      dataView.setUint32(1, numKeys);
      dataView.setUint8(0, 0xdf);
      byteParts.push(new Uint8Array(dataView.buffer));
    } else {
      throw new Error("Cannot safely encode map with size larger than 32 bits");
    }

    for (const [key, value] of Object.entries(object)) {
      encodeSlice(key, byteParts);
      encodeSlice(value, byteParts);
    }
    return;
  }

  throw new Error("Cannot safely encode value into messagepack");
}
