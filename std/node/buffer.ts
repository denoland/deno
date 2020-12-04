// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as hex from "../encoding/hex.ts";
import * as base64 from "../encoding/base64.ts";
import { Encodings, normalizeEncoding, notImplemented } from "./_utils.ts";

const notImplementedEncodings = [
  "ascii",
  "binary",
  "latin1",
  "ucs2",
  "utf16le",
];

function checkEncoding(encoding = "utf8", strict = true): Encodings {
  if (typeof encoding !== "string" || (strict && encoding === "")) {
    if (!strict) return "utf8";
    throw new TypeError(`Unkown encoding: ${encoding}`);
  }

  const normalized = normalizeEncoding(encoding);

  if (normalized === undefined) {
    throw new TypeError(`Unkown encoding: ${encoding}`);
  }

  if (notImplementedEncodings.includes(encoding)) {
    notImplemented(`"${encoding}" encoding`);
  }

  return normalized;
}

interface EncodingOp {
  byteLength(string: string): number;
}

// https://github.com/nodejs/node/blob/56dbe466fdbc598baea3bfce289bf52b97b8b8f7/lib/buffer.js#L598
const encodingOps: { [key: string]: EncodingOp } = {
  utf8: {
    byteLength: (string: string): number =>
      new TextEncoder().encode(string).byteLength,
  },
  ucs2: {
    byteLength: (string: string): number => string.length * 2,
  },
  utf16le: {
    byteLength: (string: string): number => string.length * 2,
  },
  latin1: {
    byteLength: (string: string): number => string.length,
  },
  ascii: {
    byteLength: (string: string): number => string.length,
  },
  base64: {
    byteLength: (string: string): number =>
      base64ByteLength(string, string.length),
  },
  hex: {
    byteLength: (string: string): number => string.length >>> 1,
  },
};

function base64ByteLength(str: string, bytes: number): number {
  // Handle padding
  if (str.charCodeAt(bytes - 1) === 0x3d) bytes--;
  if (bytes > 1 && str.charCodeAt(bytes - 1) === 0x3d) bytes--;

  // Base64 ratio: 3/4
  return (bytes * 3) >>> 2;
}

/**
 * See also https://nodejs.org/api/buffer.html
 */
export class Buffer extends Uint8Array {
  /**
   * Allocates a new Buffer of size bytes.
   */
  static alloc(
    size: number,
    fill?: number | string | Uint8Array | Buffer,
    encoding = "utf8",
  ): Buffer {
    if (typeof size !== "number") {
      throw new TypeError(
        `The "size" argument must be of type number. Received type ${typeof size}`,
      );
    }

    const buf = new Buffer(size);
    if (size === 0) return buf;

    let bufFill;
    if (typeof fill === "string") {
      const clearEncoding = checkEncoding(encoding);
      if (
        typeof fill === "string" &&
        fill.length === 1 &&
        clearEncoding === "utf8"
      ) {
        buf.fill(fill.charCodeAt(0));
      } else bufFill = Buffer.from(fill, clearEncoding);
    } else if (typeof fill === "number") {
      buf.fill(fill);
    } else if (fill instanceof Uint8Array) {
      if (fill.length === 0) {
        throw new TypeError(
          `The argument "value" is invalid. Received ${fill.constructor.name} []`,
        );
      }

      bufFill = fill;
    }

    if (bufFill) {
      if (bufFill.length > buf.length) {
        bufFill = bufFill.subarray(0, buf.length);
      }

      let offset = 0;
      while (offset < size) {
        buf.set(bufFill, offset);
        offset += bufFill.length;
        if (offset + bufFill.length >= size) break;
      }
      if (offset !== size) {
        buf.set(bufFill.subarray(0, size - offset), offset);
      }
    }

    return buf;
  }

  static allocUnsafe(size: number): Buffer {
    return new Buffer(size);
  }

  /**
   * Returns the byte length of a string when encoded. This is not the same as
   * String.prototype.length, which does not account for the encoding that is
   * used to convert the string into bytes.
   */
  static byteLength(
    string: string | Buffer | ArrayBufferView | ArrayBuffer | SharedArrayBuffer,
    encoding = "utf8",
  ): number {
    if (typeof string != "string") return string.byteLength;

    encoding = normalizeEncoding(encoding) || "utf8";
    return encodingOps[encoding].byteLength(string);
  }

  /**
   * Returns a new Buffer which is the result of concatenating all the Buffer
   * instances in the list together.
   */
  static concat(list: Buffer[] | Uint8Array[], totalLength?: number): Buffer {
    if (totalLength == undefined) {
      totalLength = 0;
      for (const buf of list) {
        totalLength += buf.length;
      }
    }

    const buffer = Buffer.allocUnsafe(totalLength);
    let pos = 0;
    for (const item of list) {
      let buf: Buffer;
      if (!(item instanceof Buffer)) {
        buf = Buffer.from(item);
      } else {
        buf = item;
      }
      buf.copy(buffer, pos);
      pos += buf.length;
    }

    return buffer;
  }

  /**
   * Allocates a new Buffer using an array of bytes in the range 0 – 255. Array
   * entries outside that range will be truncated to fit into it.
   */
  static from(array: number[]): Buffer;
  /**
   * This creates a view of the ArrayBuffer without copying the underlying
   * memory. For example, when passed a reference to the .buffer property of a
   * TypedArray instance, the newly created Buffer will share the same allocated
   * memory as the TypedArray.
   */
  static from(
    arrayBuffer: ArrayBuffer | SharedArrayBuffer,
    byteOffset?: number,
    length?: number,
  ): Buffer;
  /**
   * Copies the passed buffer data onto a new Buffer instance.
   */
  static from(buffer: Buffer | Uint8Array): Buffer;
  /**
   * Creates a new Buffer containing string.
   */
  static from(string: string, encoding?: string): Buffer;
  static from(
    // deno-lint-ignore no-explicit-any
    value: any,
    offsetOrEncoding?: number | string,
    length?: number,
  ): Buffer {
    const offset = typeof offsetOrEncoding === "string"
      ? undefined
      : offsetOrEncoding;
    let encoding = typeof offsetOrEncoding === "string"
      ? offsetOrEncoding
      : undefined;

    if (typeof value == "string") {
      encoding = checkEncoding(encoding, false);
      if (encoding === "hex") return new Buffer(hex.decodeString(value).buffer);
      if (encoding === "base64") return new Buffer(base64.decode(value).buffer);
      return new Buffer(new TextEncoder().encode(value).buffer);
    }

    // workaround for https://github.com/microsoft/TypeScript/issues/38446
    return new Buffer(value, offset!, length);
  }

  /**
   * Returns true if obj is a Buffer, false otherwise.
   */
  static isBuffer(obj: unknown): obj is Buffer {
    return obj instanceof Buffer;
  }

  // deno-lint-ignore no-explicit-any
  static isEncoding(encoding: any): boolean {
    return (
      typeof encoding === "string" &&
      encoding.length !== 0 &&
      normalizeEncoding(encoding) !== undefined
    );
  }

  /**
   * Copies data from a region of buf to a region in target, even if the target
   * memory region overlaps with buf.
   */
  copy(
    targetBuffer: Buffer | Uint8Array,
    targetStart = 0,
    sourceStart = 0,
    sourceEnd = this.length,
  ): number {
    const sourceBuffer = this
      .subarray(sourceStart, sourceEnd)
      .subarray(0, Math.max(0, targetBuffer.length - targetStart));

    if (sourceBuffer.length === 0) return 0;

    targetBuffer.set(sourceBuffer, targetStart);
    return sourceBuffer.length;
  }

  /*
   * Returns true if both buf and otherBuffer have exactly the same bytes, false otherwise.
   */
  equals(otherBuffer: Uint8Array | Buffer): boolean {
    if (!(otherBuffer instanceof Uint8Array)) {
      throw new TypeError(
        `The "otherBuffer" argument must be an instance of Buffer or Uint8Array. Received type ${typeof otherBuffer}`,
      );
    }

    if (this === otherBuffer) return true;
    if (this.byteLength !== otherBuffer.byteLength) return false;

    for (let i = 0; i < this.length; i++) {
      if (this[i] !== otherBuffer[i]) return false;
    }

    return true;
  }

  readBigInt64BE(offset = 0): bigint {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getBigInt64(offset);
  }
  readBigInt64LE(offset = 0): bigint {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getBigInt64(offset, true);
  }

  readBigUInt64BE(offset = 0): bigint {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getBigUint64(offset);
  }
  readBigUInt64LE(offset = 0): bigint {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getBigUint64(offset, true);
  }

  readDoubleBE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getFloat64(offset);
  }
  readDoubleLE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getFloat64(offset, true);
  }

  readFloatBE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getFloat32(offset);
  }
  readFloatLE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getFloat32(offset, true);
  }

  readInt8(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getInt8(
      offset,
    );
  }

  readInt16BE(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getInt16(
      offset,
    );
  }
  readInt16LE(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getInt16(
      offset,
      true,
    );
  }

  readInt32BE(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getInt32(
      offset,
    );
  }
  readInt32LE(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getInt32(
      offset,
      true,
    );
  }

  readUInt8(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getUint8(
      offset,
    );
  }

  readUInt16BE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getUint16(offset);
  }
  readUInt16LE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getUint16(offset, true);
  }

  readUInt32BE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getUint32(offset);
  }
  readUInt32LE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength,
    ).getUint32(offset, true);
  }

  /**
   * Returns a new Buffer that references the same memory as the original, but
   * offset and cropped by the start and end indices.
   */
  slice(begin = 0, end = this.length): Buffer {
    // workaround for https://github.com/microsoft/TypeScript/issues/38665
    return this.subarray(begin, end) as Buffer;
  }

  /**
   * Returns a JSON representation of buf. JSON.stringify() implicitly calls
   * this function when stringifying a Buffer instance.
   */
  toJSON(): Record<string, unknown> {
    return { type: "Buffer", data: Array.from(this) };
  }

  /**
   * Decodes buf to a string according to the specified character encoding in
   * encoding. start and end may be passed to decode only a subset of buf.
   */
  toString(encoding = "utf8", start = 0, end = this.length): string {
    encoding = checkEncoding(encoding);

    const b = this.subarray(start, end);
    if (encoding === "hex") return hex.encodeToString(b);
    if (encoding === "base64") return base64.encode(b.buffer);

    return new TextDecoder(encoding).decode(b);
  }

  /**
   * Writes string to buf at offset according to the character encoding in
   * encoding. The length parameter is the number of bytes to write. If buf did
   * not contain enough space to fit the entire string, only part of string will
   * be written. However, partially encoded characters will not be written.
   */
  write(string: string, offset = 0, length = this.length): number {
    return new TextEncoder().encodeInto(
      string,
      this.subarray(offset, offset + length),
    ).written;
  }

  writeBigInt64BE(value: bigint, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setBigInt64(
      offset,
      value,
    );
    return offset + 4;
  }
  writeBigInt64LE(value: bigint, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setBigInt64(
      offset,
      value,
      true,
    );
    return offset + 4;
  }

  writeBigUInt64BE(value: bigint, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setBigUint64(
      offset,
      value,
    );
    return offset + 4;
  }
  writeBigUInt64LE(value: bigint, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setBigUint64(
      offset,
      value,
      true,
    );
    return offset + 4;
  }

  writeDoubleBE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setFloat64(
      offset,
      value,
    );
    return offset + 8;
  }
  writeDoubleLE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setFloat64(
      offset,
      value,
      true,
    );
    return offset + 8;
  }

  writeFloatBE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setFloat32(
      offset,
      value,
    );
    return offset + 4;
  }
  writeFloatLE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setFloat32(
      offset,
      value,
      true,
    );
    return offset + 4;
  }

  writeInt8(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setInt8(
      offset,
      value,
    );
    return offset + 1;
  }

  writeInt16BE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setInt16(
      offset,
      value,
    );
    return offset + 2;
  }
  writeInt16LE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setInt16(
      offset,
      value,
      true,
    );
    return offset + 2;
  }

  writeInt32BE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint32(
      offset,
      value,
    );
    return offset + 4;
  }
  writeInt32LE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setInt32(
      offset,
      value,
      true,
    );
    return offset + 4;
  }

  writeUInt8(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint8(
      offset,
      value,
    );
    return offset + 1;
  }

  writeUInt16BE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint16(
      offset,
      value,
    );
    return offset + 2;
  }
  writeUInt16LE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint16(
      offset,
      value,
      true,
    );
    return offset + 2;
  }

  writeUInt32BE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint32(
      offset,
      value,
    );
    return offset + 4;
  }
  writeUInt32LE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint32(
      offset,
      value,
      true,
    );
    return offset + 4;
  }
}

export default { Buffer };
