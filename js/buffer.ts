// This code has been ported almost directly from Go's src/bytes/buffer.go
// Copyright 2009 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE

//import * as io from "./io";
import { Reader, Writer, ReadResult } from "./io";
import { assert } from "./util";
import { TextDecoder } from "./text_encoding";
import { DenoError, ErrorKind } from "./errors";

// MIN_READ is the minimum ArrayBuffer size passed to a read call by
// buffer.ReadFrom. As long as the Buffer has at least MIN_READ bytes beyond
// what is required to hold the contents of r, readFrom() will not grow the
// underlying buffer.
const MIN_READ = 512;
const MAX_SIZE = 2 ** 32 - 2;

// `off` is the offset into `dst` where it will at which to begin writing values
// from `src`.
// Returns the number of bytes copied.
function copyBytes(dst: Uint8Array, src: Uint8Array, off = 0): number {
  const r = dst.byteLength - off;
  if (src.byteLength > r) {
    src = src.subarray(0, r);
  }
  dst.set(src, off);
  return src.byteLength;
}

/** A Buffer is a variable-sized buffer of bytes with read() and write()
 * methods. Based on https://golang.org/pkg/bytes/#Buffer
 */
export class Buffer implements Reader, Writer {
  private buf: Uint8Array; // contents are the bytes buf[off : len(buf)]
  private off = 0; // read at buf[off], write at buf[buf.byteLength]

  constructor(ab?: ArrayBuffer) {
    if (ab == null) {
      this.buf = new Uint8Array(0);
    } else {
      this.buf = new Uint8Array(ab);
    }
  }

  /** bytes() returns a slice holding the unread portion of the buffer.
   * The slice is valid for use only until the next buffer modification (that
   * is, only until the next call to a method like read(), write(), reset(), or
   * truncate()). The slice aliases the buffer content at least until the next
   * buffer modification, so immediate changes to the slice will affect the
   * result of future reads.
   */
  bytes(): Uint8Array {
    return this.buf.subarray(this.off);
  }

  /** toString() returns the contents of the unread portion of the buffer
   * as a string. Warning - if multibyte characters are present when data is
   * flowing through the buffer, this method may result in incorrect strings
   * due to a character being split.
   */
  toString(): string {
    const decoder = new TextDecoder();
    return decoder.decode(this.buf.subarray(this.off));
  }

  /** empty() returns whether the unread portion of the buffer is empty. */
  empty() {
    return this.buf.byteLength <= this.off;
  }

  /** length is a getter that returns the number of bytes of the unread
   * portion of the buffer
   */
  get length() {
    return this.buf.byteLength - this.off;
  }

  /** Returns the capacity of the buffer's underlying byte slice, that is,
   * the total space allocated for the buffer's data.
   */
  get capacity(): number {
    return this.buf.buffer.byteLength;
  }

  /** truncate() discards all but the first n unread bytes from the buffer but
   * continues to use the same allocated storage.  It throws if n is negative or
   * greater than the length of the buffer.
   */
  truncate(n: number): void {
    if (n === 0) {
      this.reset();
      return;
    }
    if (n < 0 || n > this.length) {
      throw Error("bytes.Buffer: truncation out of range");
    }
    this._reslice(this.off + n);
  }

  /** reset() resets the buffer to be empty, but it retains the underlying
   * storage for use by future writes. reset() is the same as truncate(0)
   */
  reset(): void {
    this._reslice(0);
    this.off = 0;
  }

  /** _tryGrowByReslice() is a version of grow for the fast-case
   * where the internal buffer only needs to be resliced. It returns the index
   * where bytes should be written and whether it succeeded.
   * It returns -1 if a reslice was not needed.
   */
  private _tryGrowByReslice(n: number): number {
    const l = this.buf.byteLength;
    if (n <= this.capacity - l) {
      this._reslice(l + n);
      return l;
    }
    return -1;
  }

  private _reslice(len: number): void {
    assert(len <= this.buf.buffer.byteLength);
    this.buf = new Uint8Array(this.buf.buffer, 0, len);
  }

  /** read() reads the next len(p) bytes from the buffer or until the buffer
   * is drained. The return value n is the number of bytes read. If the
   * buffer has no data to return, eof in the response will be true.
   */
  async read(p: Uint8Array): Promise<ReadResult> {
    if (this.empty()) {
      // Buffer is empty, reset to recover space.
      this.reset();
      if (p.byteLength === 0) {
        // this edge case is tested in 'bufferReadEmptyAtEOF' test
        return { nread: 0, eof: false };
      }
      return { nread: 0, eof: true };
    }
    const nread = copyBytes(p, this.buf.subarray(this.off));
    this.off += nread;
    return { nread, eof: false };
  }

  async write(p: Uint8Array): Promise<number> {
    const m = this._grow(p.byteLength);
    return copyBytes(this.buf, p, m);
  }

  /** _grow() grows the buffer to guarantee space for n more bytes.
   * It returns the index where bytes should be written.
   * If the buffer can't grow it will throw with ErrTooLarge.
   */
  private _grow(n: number): number {
    const m = this.length;
    // If buffer is empty, reset to recover space.
    if (m === 0 && this.off !== 0) {
      this.reset();
    }
    // Fast: Try to grow by means of a reslice.
    const i = this._tryGrowByReslice(n);
    if (i >= 0) {
      return i;
    }
    const c = this.capacity;
    if (n <= Math.floor(c / 2) - m) {
      // We can slide things down instead of allocating a new
      // ArrayBuffer. We only need m+n <= c to slide, but
      // we instead let capacity get twice as large so we
      // don't spend all our time copying.
      copyBytes(this.buf, this.buf.subarray(this.off));
    } else if (c > MAX_SIZE - c - n) {
      throw new DenoError(
        ErrorKind.TooLarge,
        "The buffer cannot be grown beyond the maximum size."
      );
    } else {
      // Not enough space anywhere, we need to allocate.
      const buf = new Uint8Array(2 * c + n);
      copyBytes(buf, this.buf.subarray(this.off));
      this.buf = buf;
    }
    // Restore this.off and len(this.buf).
    this.off = 0;
    this._reslice(m + n);
    return m;
  }

  /** grow() grows the buffer's capacity, if necessary, to guarantee space for
   * another n bytes. After grow(n), at least n bytes can be written to the
   * buffer without another allocation. If n is negative, grow() will panic. If
   * the buffer can't grow it will throw ErrTooLarge.
   * Based on https://golang.org/pkg/bytes/#Buffer.Grow
   */
  grow(n: number): void {
    if (n < 0) {
      throw Error("Buffer.grow: negative count");
    }
    const m = this._grow(n);
    this._reslice(m);
  }

  /** readFrom() reads data from r until EOF and appends it to the buffer,
   * growing the buffer as needed. It returns the number of bytes read. If the
   * buffer becomes too large, readFrom will panic with ErrTooLarge.
   * Based on https://golang.org/pkg/bytes/#Buffer.ReadFrom
   */
  async readFrom(r: Reader): Promise<number> {
    let n = 0;
    while (true) {
      try {
        const i = this._grow(MIN_READ);
        this._reslice(i);
        const fub = new Uint8Array(this.buf.buffer, i);
        const { nread, eof } = await r.read(fub);
        this._reslice(i + nread);
        n += nread;
        if (eof) {
          return n;
        }
      } catch (e) {
        return n;
      }
    }
  }
}

/** Read `r` until EOF and return the content as `Uint8Array`.
 */
export async function readAll(r: Reader): Promise<Uint8Array> {
  const buf = new Buffer();
  await buf.readFrom(r);
  return buf.bytes();
}
