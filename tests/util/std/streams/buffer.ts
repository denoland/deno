// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { assert } from "../assert/assert.ts";
import { copy } from "../bytes/copy.ts";

const MAX_SIZE = 2 ** 32 - 2;
const DEFAULT_CHUNK_SIZE = 16_640;

/** A variable-sized buffer of bytes with `read()` and `write()` methods.
 *
 * Buffer is almost always used with some I/O like files and sockets. It allows
 * one to buffer up a download from a socket. Buffer grows and shrinks as
 * necessary.
 *
 * Buffer is NOT the same thing as Node's Buffer. Node's Buffer was created in
 * 2009 before JavaScript had the concept of ArrayBuffers. It's simply a
 * non-standard ArrayBuffer.
 *
 * ArrayBuffer is a fixed memory allocation. Buffer is implemented on top of
 * ArrayBuffer.
 *
 * Based on [Go Buffer](https://golang.org/pkg/bytes/#Buffer). */
export class Buffer {
  #buf: Uint8Array; // contents are the bytes buf[off : len(buf)]
  #off = 0; // read at buf[off], write at buf[buf.byteLength]
  #readable: ReadableStream<Uint8Array> = new ReadableStream({
    type: "bytes",
    pull: (controller) => {
      const view = new Uint8Array(controller.byobRequest!.view!.buffer);
      if (this.empty()) {
        // Buffer is empty, reset to recover space.
        this.reset();
        controller.close();
        controller.byobRequest!.respond(0);
        return;
      }
      const nread = copy(this.#buf.subarray(this.#off), view);
      this.#off += nread;
      controller.byobRequest!.respond(nread);
    },
    autoAllocateChunkSize: DEFAULT_CHUNK_SIZE,
  });
  get readable() {
    return this.#readable;
  }
  #writable = new WritableStream<Uint8Array>({
    write: (chunk) => {
      const m = this.#grow(chunk.byteLength);
      copy(chunk, this.#buf, m);
    },
  });
  get writable() {
    return this.#writable;
  }

  constructor(ab?: ArrayBufferLike | ArrayLike<number>) {
    this.#buf = ab === undefined ? new Uint8Array(0) : new Uint8Array(ab);
  }

  /** Returns a slice holding the unread portion of the buffer.
   *
   * The slice is valid for use only until the next buffer modification (that
   * is, only until the next call to a method like `read()`, `write()`,
   * `reset()`, or `truncate()`). If `options.copy` is false the slice aliases
   * the buffer content at least until the next buffer modification, so
   * immediate changes to the slice will affect the result of future reads.
   */
  bytes(options = { copy: true }): Uint8Array {
    if (options.copy === false) return this.#buf.subarray(this.#off);
    return this.#buf.slice(this.#off);
  }

  /** Returns whether the unread portion of the buffer is empty. */
  empty(): boolean {
    return this.#buf.byteLength <= this.#off;
  }

  /** A read only number of bytes of the unread portion of the buffer. */
  get length(): number {
    return this.#buf.byteLength - this.#off;
  }

  /** The read only capacity of the buffer's underlying byte slice, that is,
   * the total space allocated for the buffer's data. */
  get capacity(): number {
    return this.#buf.buffer.byteLength;
  }

  /** Discards all but the first `n` unread bytes from the buffer but
   * continues to use the same allocated storage. It throws if `n` is
   * negative or greater than the length of the buffer. */
  truncate(n: number) {
    if (n === 0) {
      this.reset();
      return;
    }
    if (n < 0 || n > this.length) {
      throw Error("bytes.Buffer: truncation out of range");
    }
    this.#reslice(this.#off + n);
  }

  reset() {
    this.#reslice(0);
    this.#off = 0;
  }

  #tryGrowByReslice(n: number) {
    const l = this.#buf.byteLength;
    if (n <= this.capacity - l) {
      this.#reslice(l + n);
      return l;
    }
    return -1;
  }

  #reslice(len: number) {
    assert(len <= this.#buf.buffer.byteLength);
    this.#buf = new Uint8Array(this.#buf.buffer, 0, len);
  }

  #grow(n: number) {
    const m = this.length;
    // If buffer is empty, reset to recover space.
    if (m === 0 && this.#off !== 0) {
      this.reset();
    }
    // Fast: Try to grow by means of a reslice.
    const i = this.#tryGrowByReslice(n);
    if (i >= 0) {
      return i;
    }
    const c = this.capacity;
    if (n <= Math.floor(c / 2) - m) {
      // We can slide things down instead of allocating a new
      // ArrayBuffer. We only need m+n <= c to slide, but
      // we instead let capacity get twice as large so we
      // don't spend all our time copying.
      copy(this.#buf.subarray(this.#off), this.#buf);
    } else if (c + n > MAX_SIZE) {
      throw new Error("The buffer cannot be grown beyond the maximum size.");
    } else {
      // Not enough space anywhere, we need to allocate.
      const buf = new Uint8Array(Math.min(2 * c + n, MAX_SIZE));
      copy(this.#buf.subarray(this.#off), buf);
      this.#buf = buf;
    }
    // Restore this.#off and len(this.#buf).
    this.#off = 0;
    this.#reslice(Math.min(m + n, MAX_SIZE));
    return m;
  }

  /** Grows the buffer's capacity, if necessary, to guarantee space for
   * another `n` bytes. After `.grow(n)`, at least `n` bytes can be written to
   * the buffer without another allocation. If `n` is negative, `.grow()` will
   * throw. If the buffer can't grow it will throw an error.
   *
   * Based on Go Lang's
   * [Buffer.Grow](https://golang.org/pkg/bytes/#Buffer.Grow). */
  grow(n: number) {
    if (n < 0) {
      throw Error("Buffer.grow: negative count");
    }
    const m = this.#grow(n);
    this.#reslice(m);
  }
}
