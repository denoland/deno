// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { copy } from "../bytes/copy.ts";
import type { Writer, WriterSync } from "../types.d.ts";

const DEFAULT_BUF_SIZE = 4096;

abstract class AbstractBufBase {
  buf: Uint8Array;
  usedBufferBytes = 0;
  err: Error | null = null;

  constructor(buf: Uint8Array) {
    this.buf = buf;
  }

  /** Size returns the size of the underlying buffer in bytes. */
  size(): number {
    return this.buf.byteLength;
  }

  /** Returns how many bytes are unused in the buffer. */
  available(): number {
    return this.buf.byteLength - this.usedBufferBytes;
  }

  /** buffered returns the number of bytes that have been written into the
   * current buffer.
   */
  buffered(): number {
    return this.usedBufferBytes;
  }
}

/** BufWriter implements buffering for an deno.Writer object.
 * If an error occurs writing to a Writer, no more data will be
 * accepted and all subsequent writes, and flush(), will return the error.
 * After all data has been written, the client should call the
 * flush() method to guarantee all data has been forwarded to
 * the underlying deno.Writer.
 *
 * @deprecated (will be removed after 1.0.0) Use the [Web Streams API]{@link https://developer.mozilla.org/en-US/docs/Web/API/Streams_API} instead.
 */
export class BufWriter extends AbstractBufBase implements Writer {
  #writer: Writer;

  /** return new BufWriter unless writer is BufWriter */
  static create(writer: Writer, size: number = DEFAULT_BUF_SIZE): BufWriter {
    return writer instanceof BufWriter ? writer : new BufWriter(writer, size);
  }

  constructor(writer: Writer, size: number = DEFAULT_BUF_SIZE) {
    super(new Uint8Array(size <= 0 ? DEFAULT_BUF_SIZE : size));
    this.#writer = writer;
  }

  /** Discards any unflushed buffered data, clears any error, and
   * resets buffer to write its output to w.
   */
  reset(w: Writer) {
    this.err = null;
    this.usedBufferBytes = 0;
    this.#writer = w;
  }

  /** Flush writes any buffered data to the underlying io.Writer. */
  async flush() {
    if (this.err !== null) throw this.err;
    if (this.usedBufferBytes === 0) return;

    try {
      const p = this.buf.subarray(0, this.usedBufferBytes);
      let nwritten = 0;
      while (nwritten < p.length) {
        nwritten += await this.#writer.write(p.subarray(nwritten));
      }
    } catch (e) {
      if (e instanceof Error) {
        this.err = e;
      }
      throw e;
    }

    this.buf = new Uint8Array(this.buf.length);
    this.usedBufferBytes = 0;
  }

  /** Writes the contents of `data` into the buffer. If the contents won't fully
   * fit into the buffer, those bytes that are copied into the buffer will be flushed
   * to the writer and the remaining bytes are then copied into the now empty buffer.
   *
   * @return the number of bytes written to the buffer.
   */
  async write(data: Uint8Array): Promise<number> {
    if (this.err !== null) throw this.err;
    if (data.length === 0) return 0;

    let totalBytesWritten = 0;
    let numBytesWritten = 0;
    while (data.byteLength > this.available()) {
      if (this.buffered() === 0) {
        // Large write, empty buffer.
        // Write directly from data to avoid copy.
        try {
          numBytesWritten = await this.#writer.write(data);
        } catch (e) {
          if (e instanceof Error) {
            this.err = e;
          }
          throw e;
        }
      } else {
        numBytesWritten = copy(data, this.buf, this.usedBufferBytes);
        this.usedBufferBytes += numBytesWritten;
        await this.flush();
      }
      totalBytesWritten += numBytesWritten;
      data = data.subarray(numBytesWritten);
    }

    numBytesWritten = copy(data, this.buf, this.usedBufferBytes);
    this.usedBufferBytes += numBytesWritten;
    totalBytesWritten += numBytesWritten;
    return totalBytesWritten;
  }
}

/** BufWriterSync implements buffering for a deno.WriterSync object.
 * If an error occurs writing to a WriterSync, no more data will be
 * accepted and all subsequent writes, and flush(), will return the error.
 * After all data has been written, the client should call the
 * flush() method to guarantee all data has been forwarded to
 * the underlying deno.WriterSync.
 *
 * @deprecated (will be removed after 1.0.0) Use the [Web Streams API]{@link https://developer.mozilla.org/en-US/docs/Web/API/Streams_API} instead.
 */
export class BufWriterSync extends AbstractBufBase implements WriterSync {
  #writer: WriterSync;

  /** return new BufWriterSync unless writer is BufWriterSync */
  static create(
    writer: WriterSync,
    size: number = DEFAULT_BUF_SIZE,
  ): BufWriterSync {
    return writer instanceof BufWriterSync
      ? writer
      : new BufWriterSync(writer, size);
  }

  constructor(writer: WriterSync, size: number = DEFAULT_BUF_SIZE) {
    super(new Uint8Array(size <= 0 ? DEFAULT_BUF_SIZE : size));
    this.#writer = writer;
  }

  /** Discards any unflushed buffered data, clears any error, and
   * resets buffer to write its output to w.
   */
  reset(w: WriterSync) {
    this.err = null;
    this.usedBufferBytes = 0;
    this.#writer = w;
  }

  /** Flush writes any buffered data to the underlying io.WriterSync. */
  flush() {
    if (this.err !== null) throw this.err;
    if (this.usedBufferBytes === 0) return;

    try {
      const p = this.buf.subarray(0, this.usedBufferBytes);
      let nwritten = 0;
      while (nwritten < p.length) {
        nwritten += this.#writer.writeSync(p.subarray(nwritten));
      }
    } catch (e) {
      if (e instanceof Error) {
        this.err = e;
      }
      throw e;
    }

    this.buf = new Uint8Array(this.buf.length);
    this.usedBufferBytes = 0;
  }

  /** Writes the contents of `data` into the buffer.  If the contents won't fully
   * fit into the buffer, those bytes that can are copied into the buffer, the
   * buffer is the flushed to the writer and the remaining bytes are copied into
   * the now empty buffer.
   *
   * @return the number of bytes written to the buffer.
   */
  writeSync(data: Uint8Array): number {
    if (this.err !== null) throw this.err;
    if (data.length === 0) return 0;

    let totalBytesWritten = 0;
    let numBytesWritten = 0;
    while (data.byteLength > this.available()) {
      if (this.buffered() === 0) {
        // Large write, empty buffer.
        // Write directly from data to avoid copy.
        try {
          numBytesWritten = this.#writer.writeSync(data);
        } catch (e) {
          if (e instanceof Error) {
            this.err = e;
          }
          throw e;
        }
      } else {
        numBytesWritten = copy(data, this.buf, this.usedBufferBytes);
        this.usedBufferBytes += numBytesWritten;
        this.flush();
      }
      totalBytesWritten += numBytesWritten;
      data = data.subarray(numBytesWritten);
    }

    numBytesWritten = copy(data, this.buf, this.usedBufferBytes);
    this.usedBufferBytes += numBytesWritten;
    totalBytesWritten += numBytesWritten;
    return totalBytesWritten;
  }
}
