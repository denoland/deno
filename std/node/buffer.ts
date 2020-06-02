/**
 * See also https://nodejs.org/api/buffer.html
 */
export default class Buffer extends Uint8Array {
  /**
   * Allocates a new Buffer of size bytes.
   */
  static alloc(size: number): Buffer {
    return new Buffer(size);
  }

  /**
   * Returns the byte length of a string when encoded. This is not the same as
   * String.prototype.length, which does not account for the encoding that is
   * used to convert the string into bytes.
   */
  static byteLength(
    string: string | Buffer | ArrayBufferView | ArrayBuffer | SharedArrayBuffer
  ): number {
    if (typeof string != "string") return string.byteLength;
    return new TextEncoder().encode(string).length;
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

    const buffer = new Buffer(totalLength);
    let pos = 0;
    for (const buf of list) {
      buffer.set(buf, pos);
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
    length?: number
  ): Buffer;
  /**
   * Copies the passed buffer data onto a new Buffer instance.
   */
  static from(buffer: Buffer | Uint8Array): Buffer;
  /**
   * Creates a new Buffer containing string.
   */
  static from(string: string): Buffer;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  static from(value: any, offset?: number, length?: number): Buffer {
    if (typeof value == "string")
      return new Buffer(new TextEncoder().encode(value).buffer);

    // workaround for https://github.com/microsoft/TypeScript/issues/38446
    return new Buffer(value, offset!, length);
  }

  /**
   * Returns true if obj is a Buffer, false otherwise.
   */
  static isBuffer(obj: object): obj is Buffer {
    return obj instanceof Buffer;
  }

  /**
   * Copies data from a region of buf to a region in target, even if the target
   * memory region overlaps with buf.
   */
  copy(
    targetBuffer: Buffer | Uint8Array,
    targetStart = 0,
    sourceStart = 0,
    sourceEnd = this.length
  ): number {
    const sourceBuffer = this.subarray(sourceStart, sourceEnd);
    targetBuffer.set(sourceBuffer, targetStart);
    return sourceBuffer.length;
  }

  readBigInt64BE(offset = 0): bigint {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
    ).getBigInt64(offset);
  }
  readBigInt64LE(offset = 0): bigint {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
    ).getBigInt64(offset, true);
  }

  readBigUInt64BE(offset = 0): bigint {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
    ).getBigUint64(offset);
  }
  readBigUInt64LE(offset = 0): bigint {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
    ).getBigUint64(offset, true);
  }

  readDoubleBE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
    ).getFloat64(offset);
  }
  readDoubleLE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
    ).getFloat64(offset, true);
  }

  readFloatBE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
    ).getFloat32(offset);
  }
  readFloatLE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
    ).getFloat32(offset, true);
  }

  readInt8(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getInt8(
      offset
    );
  }

  readInt16BE(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getInt16(
      offset
    );
  }
  readInt16LE(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getInt16(
      offset,
      true
    );
  }

  readInt32BE(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getInt32(
      offset
    );
  }
  readInt32LE(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getInt32(
      offset,
      true
    );
  }

  readUInt8(offset = 0): number {
    return new DataView(this.buffer, this.byteOffset, this.byteLength).getUint8(
      offset
    );
  }

  readUInt16BE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
    ).getUint16(offset);
  }
  readUInt16LE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
    ).getUint16(offset, true);
  }

  readUInt32BE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
    ).getUint32(offset);
  }
  readUInt32LE(offset = 0): number {
    return new DataView(
      this.buffer,
      this.byteOffset,
      this.byteLength
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
  toJSON(): object {
    return { type: "Buffer", data: Array.from(this) };
  }

  /**
   * Decodes buf to a string according to the specified character encoding in
   * encoding. start and end may be passed to decode only a subset of buf.
   */
  toString(encoding = "utf8", start = 0, end = this.length): string {
    return new TextDecoder(encoding).decode(this.subarray(start, end));
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
      this.subarray(offset, offset + length)
    ).written;
  }

  writeBigInt64BE(value: bigint, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setBigInt64(
      offset,
      value
    );
    return offset + 4;
  }
  writeBigInt64LE(value: bigint, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setBigInt64(
      offset,
      value,
      true
    );
    return offset + 4;
  }

  writeBigUInt64BE(value: bigint, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setBigUint64(
      offset,
      value
    );
    return offset + 4;
  }
  writeBigUInt64LE(value: bigint, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setBigUint64(
      offset,
      value,
      true
    );
    return offset + 4;
  }

  writeDoubleBE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setFloat64(
      offset,
      value
    );
    return offset + 8;
  }
  writeDoubleLE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setFloat64(
      offset,
      value,
      true
    );
    return offset + 8;
  }

  writeFloatBE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setFloat32(
      offset,
      value
    );
    return offset + 4;
  }
  writeFloatLE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setFloat32(
      offset,
      value,
      true
    );
    return offset + 4;
  }

  writeInt8(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setInt8(
      offset,
      value
    );
    return offset + 1;
  }

  writeInt16BE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setInt16(
      offset,
      value
    );
    return offset + 2;
  }
  writeInt16LE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setInt16(
      offset,
      value,
      true
    );
    return offset + 2;
  }

  writeInt32BE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint32(
      offset,
      value
    );
    return offset + 4;
  }
  writeInt32LE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setInt32(
      offset,
      value,
      true
    );
    return offset + 4;
  }

  writeUInt8(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint8(
      offset,
      value
    );
    return offset + 1;
  }

  writeUInt16BE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint16(
      offset,
      value
    );
    return offset + 2;
  }
  writeUInt16LE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint16(
      offset,
      value,
      true
    );
    return offset + 2;
  }

  writeUInt32BE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint32(
      offset,
      value
    );
    return offset + 4;
  }
  writeUInt32LE(value: number, offset = 0): number {
    new DataView(this.buffer, this.byteOffset, this.byteLength).setUint32(
      offset,
      value,
      true
    );
    return offset + 4;
  }
}

export { Buffer };

Object.defineProperty(globalThis, "Buffer", {
  value: Buffer,
  enumerable: false,
  writable: true,
  configurable: true,
});
