// Based on https://github.com/golang/go/blob/891682/src/bufio/bufio.go
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

type Reader = Deno.Reader;
type ReadResult = Deno.ReadResult;
type Writer = Deno.Writer;
import { charCode, copyBytes } from "./util.ts";
import { assert } from "../testing/asserts.ts";

const DEFAULT_BUF_SIZE = 4096;
const MIN_BUF_SIZE = 16;
const MAX_CONSECUTIVE_EMPTY_READS = 100;
const CR = charCode("\r");
const LF = charCode("\n");

export type BufState =
  | null
  | "EOF"
  | "BufferFull"
  | "ShortWrite"
  | "NoProgress"
  | Error;

/** BufReader implements buffering for a Reader object. */
export class BufReader implements Reader {
  private buf: Uint8Array;
  private rd: Reader; // Reader provided by caller.
  private r = 0; // buf read position.
  private w = 0; // buf write position.
  private lastByte: number;
  private lastCharSize: number;
  private err: BufState;

  constructor(rd: Reader, size = DEFAULT_BUF_SIZE) {
    if (size < MIN_BUF_SIZE) {
      size = MIN_BUF_SIZE;
    }
    this._reset(new Uint8Array(size), rd);
  }

  /** Returns the size of the underlying buffer in bytes. */
  size(): number {
    return this.buf.byteLength;
  }

  buffered(): number {
    return this.w - this.r;
  }

  private _readErr(): BufState {
    const err = this.err;
    this.err = null;
    return err;
  }

  // Reads a new chunk into the buffer.
  private async _fill(): Promise<void> {
    // Slide existing data to beginning.
    if (this.r > 0) {
      this.buf.copyWithin(0, this.r, this.w);
      this.w -= this.r;
      this.r = 0;
    }

    if (this.w >= this.buf.byteLength) {
      throw Error("bufio: tried to fill full buffer");
    }

    // Read new data: try a limited number of times.
    for (let i = MAX_CONSECUTIVE_EMPTY_READS; i > 0; i--) {
      let rr: ReadResult;
      try {
        rr = await this.rd.read(this.buf.subarray(this.w));
      } catch (e) {
        this.err = e;
        return;
      }
      assert(rr.nread >= 0, "negative read");
      this.w += rr.nread;
      if (rr.eof) {
        this.err = "EOF";
        return;
      }
      if (rr.nread > 0) {
        return;
      }
    }
    this.err = "NoProgress";
  }

  /** Discards any buffered data, resets all state, and switches
   * the buffered reader to read from r.
   */
  reset(r: Reader): void {
    this._reset(this.buf, r);
  }

  private _reset(buf: Uint8Array, rd: Reader): void {
    this.buf = buf;
    this.rd = rd;
    this.lastByte = -1;
    // this.lastRuneSize = -1;
  }

  /** reads data into p.
   * It returns the number of bytes read into p.
   * The bytes are taken from at most one Read on the underlying Reader,
   * hence n may be less than len(p).
   * At EOF, the count will be zero and err will be io.EOF.
   * To read exactly len(p) bytes, use io.ReadFull(b, p).
   */
  async read(p: Uint8Array): Promise<ReadResult> {
    let rr: ReadResult = { nread: p.byteLength, eof: false };
    if (rr.nread === 0) {
      if (this.err) {
        throw this._readErr();
      }
      return rr;
    }

    if (this.r === this.w) {
      if (this.err) {
        throw this._readErr();
      }
      if (p.byteLength >= this.buf.byteLength) {
        // Large read, empty buffer.
        // Read directly into p to avoid copy.
        rr = await this.rd.read(p);
        assert(rr.nread >= 0, "negative read");
        if (rr.nread > 0) {
          this.lastByte = p[rr.nread - 1];
          // this.lastRuneSize = -1;
        }
        if (this.err) {
          throw this._readErr();
        }
        return rr;
      }
      // One read.
      // Do not use this.fill, which will loop.
      this.r = 0;
      this.w = 0;
      try {
        rr = await this.rd.read(this.buf);
      } catch (e) {
        this.err = e;
      }
      assert(rr.nread >= 0, "negative read");
      if (rr.nread === 0) {
        if (this.err) {
          throw this._readErr();
        }
        return rr;
      }
      this.w += rr.nread;
    }

    // copy as much as we can
    rr.nread = copyBytes(p as Uint8Array, this.buf.subarray(this.r, this.w), 0);
    this.r += rr.nread;
    this.lastByte = this.buf[this.r - 1];
    // this.lastRuneSize = -1;
    return rr;
  }

  /** reads exactly len(p) bytes into p.
   * Ported from https://golang.org/pkg/io/#ReadFull
   * It returns the number of bytes copied and an error if fewer bytes were read.
   * The error is EOF only if no bytes were read.
   * If an EOF happens after reading some but not all the bytes,
   * readFull returns ErrUnexpectedEOF. ("EOF" for current impl)
   * On return, n == len(p) if and only if err == nil.
   * If r returns an error having read at least len(buf) bytes,
   * the error is dropped.
   */
  async readFull(p: Uint8Array): Promise<[number, BufState]> {
    let rr = await this.read(p);
    let nread = rr.nread;
    if (rr.eof) {
      return [nread, nread < p.length ? "EOF" : null];
    }
    while (!rr.eof && nread < p.length) {
      rr = await this.read(p.subarray(nread));
      nread += rr.nread;
    }
    return [nread, nread < p.length ? "EOF" : null];
  }

  /** Returns the next byte [0, 255] or -1 if EOF. */
  async readByte(): Promise<number> {
    while (this.r === this.w) {
      await this._fill(); // buffer is empty.
      if (this.err == "EOF") {
        return -1;
      }
      if (this.err != null) {
        throw this._readErr();
      }
    }
    const c = this.buf[this.r];
    this.r++;
    this.lastByte = c;
    return c;
  }

  /** readString() reads until the first occurrence of delim in the input,
   * returning a string containing the data up to and including the delimiter.
   * If ReadString encounters an error before finding a delimiter,
   * it returns the data read before the error and the error itself (often io.EOF).
   * ReadString returns err != nil if and only if the returned data does not end in
   * delim.
   * For simple uses, a Scanner may be more convenient.
   */
  async readString(_delim: string): Promise<string> {
    throw new Error("Not implemented");
  }

  /** readLine() is a low-level line-reading primitive. Most callers should use
   * readBytes('\n') or readString('\n') instead or use a Scanner.
   *
   * readLine tries to return a single line, not including the end-of-line bytes.
   * If the line was too long for the buffer then isPrefix is set and the
   * beginning of the line is returned. The rest of the line will be returned
   * from future calls. isPrefix will be false when returning the last fragment
   * of the line. The returned buffer is only valid until the next call to
   * ReadLine. ReadLine either returns a non-nil line or it returns an error,
   * never both.
   *
   * The text returned from ReadLine does not include the line end ("\r\n" or "\n").
   * No indication or error is given if the input ends without a final line end.
   * Calling UnreadByte after ReadLine will always unread the last byte read
   * (possibly a character belonging to the line end) even if that byte is not
   * part of the line returned by ReadLine.
   */
  async readLine(): Promise<[Uint8Array, boolean, BufState]> {
    let [line, err] = await this.readSlice(LF);

    if (err === "BufferFull") {
      // Handle the case where "\r\n" straddles the buffer.
      if (line.byteLength > 0 && line[line.byteLength - 1] === CR) {
        // Put the '\r' back on buf and drop it from line.
        // Let the next call to ReadLine check for "\r\n".
        assert(this.r > 0, "bufio: tried to rewind past start of buffer");
        this.r--;
        line = line.subarray(0, line.byteLength - 1);
      }
      return [line, true, null];
    }

    if (line.byteLength === 0) {
      return [line, false, err];
    }
    err = null;

    if (line[line.byteLength - 1] == LF) {
      let drop = 1;
      if (line.byteLength > 1 && line[line.byteLength - 2] === CR) {
        drop = 2;
      }
      line = line.subarray(0, line.byteLength - drop);
    }
    return [line, false, err];
  }

  /** readSlice() reads until the first occurrence of delim in the input,
   * returning a slice pointing at the bytes in the buffer. The bytes stop
   * being valid at the next read. If readSlice() encounters an error before
   * finding a delimiter, it returns all the data in the buffer and the error
   * itself (often io.EOF).  readSlice() fails with error ErrBufferFull if the
   * buffer fills without a delim. Because the data returned from readSlice()
   * will be overwritten by the next I/O operation, most clients should use
   * readBytes() or readString() instead. readSlice() returns err != nil if and
   * only if line does not end in delim.
   */
  async readSlice(delim: number): Promise<[Uint8Array, BufState]> {
    let s = 0; // search start index
    let line: Uint8Array;
    let err: BufState;
    while (true) {
      // Search buffer.
      let i = this.buf.subarray(this.r + s, this.w).indexOf(delim);
      if (i >= 0) {
        i += s;
        line = this.buf.subarray(this.r, this.r + i + 1);
        this.r += i + 1;
        break;
      }

      // Pending error?
      if (this.err) {
        line = this.buf.subarray(this.r, this.w);
        this.r = this.w;
        err = this._readErr();
        break;
      }

      // Buffer full?
      if (this.buffered() >= this.buf.byteLength) {
        this.r = this.w;
        line = this.buf;
        err = "BufferFull";
        break;
      }

      s = this.w - this.r; // do not rescan area we scanned before

      await this._fill(); // buffer is not full
    }

    // Handle last byte, if any.
    let i = line.byteLength - 1;
    if (i >= 0) {
      this.lastByte = line[i];
      // this.lastRuneSize = -1
    }

    return [line, err];
  }

  /** Peek returns the next n bytes without advancing the reader. The bytes stop
   * being valid at the next read call. If Peek returns fewer than n bytes, it
   * also returns an error explaining why the read is short. The error is
   * ErrBufferFull if n is larger than b's buffer size.
   */
  async peek(n: number): Promise<[Uint8Array, BufState]> {
    if (n < 0) {
      throw Error("negative count");
    }

    while (
      this.w - this.r < n &&
      this.w - this.r < this.buf.byteLength &&
      this.err == null
    ) {
      await this._fill(); // this.w - this.r < len(this.buf) => buffer is not full
    }

    if (n > this.buf.byteLength) {
      return [this.buf.subarray(this.r, this.w), "BufferFull"];
    }

    // 0 <= n <= len(this.buf)
    let err: BufState;
    let avail = this.w - this.r;
    if (avail < n) {
      // not enough data in buffer
      n = avail;
      err = this._readErr();
      if (!err) {
        err = "BufferFull";
      }
    }
    return [this.buf.subarray(this.r, this.r + n), err];
  }
}

/** BufWriter implements buffering for an deno.Writer object.
 * If an error occurs writing to a Writer, no more data will be
 * accepted and all subsequent writes, and flush(), will return the error.
 * After all data has been written, the client should call the
 * flush() method to guarantee all data has been forwarded to
 * the underlying deno.Writer.
 */
export class BufWriter implements Writer {
  buf: Uint8Array;
  n: number = 0;
  err: null | BufState = null;

  constructor(private wr: Writer, size = DEFAULT_BUF_SIZE) {
    if (size <= 0) {
      size = DEFAULT_BUF_SIZE;
    }
    this.buf = new Uint8Array(size);
  }

  /** Size returns the size of the underlying buffer in bytes. */
  size(): number {
    return this.buf.byteLength;
  }

  /** Discards any unflushed buffered data, clears any error, and
   * resets b to write its output to w.
   */
  reset(w: Writer): void {
    this.err = null;
    this.n = 0;
    this.wr = w;
  }

  /** Flush writes any buffered data to the underlying io.Writer. */
  async flush(): Promise<BufState> {
    if (this.err != null) {
      return this.err;
    }
    if (this.n == 0) {
      return null;
    }

    let n: number;
    let err: BufState = null;
    try {
      n = await this.wr.write(this.buf.subarray(0, this.n));
    } catch (e) {
      err = e;
    }

    if (n < this.n && err == null) {
      err = "ShortWrite";
    }

    if (err != null) {
      if (n > 0 && n < this.n) {
        this.buf.copyWithin(0, n, this.n);
      }
      this.n -= n;
      this.err = err;
      return err;
    }
    this.n = 0;
  }

  /** Returns how many bytes are unused in the buffer. */
  available(): number {
    return this.buf.byteLength - this.n;
  }

  /** buffered returns the number of bytes that have been written into the
   * current buffer.
   */
  buffered(): number {
    return this.n;
  }

  /** Writes the contents of p into the buffer.
   * Returns the number of bytes written.
   */
  async write(p: Uint8Array): Promise<number> {
    let nn = 0;
    let n: number;
    while (p.byteLength > this.available() && !this.err) {
      if (this.buffered() == 0) {
        // Large write, empty buffer.
        // Write directly from p to avoid copy.
        try {
          n = await this.wr.write(p);
        } catch (e) {
          this.err = e;
        }
      } else {
        n = copyBytes(this.buf, p, this.n);
        this.n += n;
        await this.flush();
      }
      nn += n;
      p = p.subarray(n);
    }
    if (this.err) {
      throw this.err;
    }
    n = copyBytes(this.buf, p, this.n);
    this.n += n;
    nn += n;
    return nn;
  }
}
