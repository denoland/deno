// Ported to Deno from:
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import { Reader, ReadResult } from "deno";
import { assert, copyBytes } from "./util.ts";

const DEFAULT_BUF_SIZE = 4096;
const MIN_BUF_SIZE = 16;
const MAX_CONSECUTIVE_EMPTY_READS = 100;

export class ErrNegativeRead extends Error {
  constructor() {
    super("bufio: reader returned negative count from Read");
    this.name = "ErrNegativeRead";
  }
}

/** BufReader implements buffering for a Reader object. */
export class BufReader implements Reader {
  private buf: Uint8Array;
  private rd: Reader; // Reader provided by caller.
  private r = 0; // buf read position.
  private w = 0; // buf write position.
  private lastByte: number;
  private lastCharSize: number;
  private err: null | Error;

  constructor(rd: Reader, size = DEFAULT_BUF_SIZE) {
    if (size < MIN_BUF_SIZE) {
      size = MIN_BUF_SIZE;
    }
    this._reset(new Uint8Array(size), rd);
  }

  /** Returns the size of the underlying buffer in bytes. */
  get byteLength(): number {
    return this.buf.byteLength;
  }

  buffered(): number {
    return this.w - this.r;
  }

  private _readErr(): Error {
    const err = this.err;
    this.err = null;
    return err;
  }

  // Reads a new chunk into the buffer.
  // Returns true if EOF, false on successful read.
  private async _fill(): Promise<boolean> {
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
        return false;
      }
      if (rr.nread < 0) {
        throw new ErrNegativeRead();
      }
      this.w += rr.nread;
      if (rr.eof) {
        return true;
      }
      if (rr.nread > 0) {
        return false;
      }
    }
    throw Error("No Progress");
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
    this.lastCharSize = -1;
  }

  /** reads data into p.
   * It returns the number of bytes read into p.
   * The bytes are taken from at most one Read on the underlying Reader,
   * hence n may be less than len(p).
   * At EOF, the count will be zero and err will be io.EOF.
   * To read exactly len(p) bytes, use io.ReadFull(b, p).
   */
  async read(p: ArrayBufferView): Promise<ReadResult> {
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
        if (rr.nread < 0) {
          throw new ErrNegativeRead();
        }
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
      rr = await this.rd.read(this.buf);
      if (rr.nread < 0) {
        throw new ErrNegativeRead();
      }
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

  /** Returns the next byte [0, 255] or -1 if EOF. */
  async readByte(): Promise<number> {
    while (this.r === this.w) {
      const eof = await this._fill(); // buffer is empty.
      if (this.err != null) {
        throw this._readErr();
      }
      if (eof) {
        return -1;
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
  async readString(delim: string): Promise<string> {
    throw new Error("Not implemented");
  }
}
