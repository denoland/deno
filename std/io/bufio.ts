// Based on https://github.com/golang/go/blob/891682/src/bufio/bufio.go
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

type Reader = Deno.Reader;
type Writer = Deno.Writer;
type WriterSync = Deno.WriterSync;
import { charCode, copyBytes } from "./util.ts";
import { assert } from "../_util/assert.ts";

const DEFAULT_BUF_SIZE = 4096;
const MIN_BUF_SIZE = 16;
const MAX_CONSECUTIVE_EMPTY_READS = 100;
const CR = charCode("\r");
const LF = charCode("\n");

export class BufferFullError extends Error {
  name = "BufferFullError";
  constructor(public partial: Uint8Array) {
    super("Buffer full");
  }
}

export class PartialReadError extends Deno.errors.UnexpectedEof {
  name = "PartialReadError";
  partial?: Uint8Array;
  constructor() {
    super("Encountered UnexpectedEof, data only partially read");
  }
}

/** Result type returned by of BufReader.readLine(). */
export interface ReadLineResult {
  line: Uint8Array;
  more: boolean;
}

/** BufReader implements buffering for a Reader object. */
export class BufReader implements Reader {
  private buf!: Uint8Array;
  private rd!: Reader; // Reader provided by caller.
  private r = 0; // buf read position.
  private w = 0; // buf write position.
  private eof = false;
  // private lastByte: number;
  // private lastCharSize: number;

  /** return new BufReader unless r is BufReader */
  static create(r: Reader, size: number = DEFAULT_BUF_SIZE): BufReader {
    return r instanceof BufReader ? r : new BufReader(r, size);
  }

  constructor(rd: Reader, size: number = DEFAULT_BUF_SIZE) {
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
      const rr = await this.rd.read(this.buf.subarray(this.w));
      if (rr === null) {
        this.eof = true;
        return;
      }
      assert(rr >= 0, "negative read");
      this.w += rr;
      if (rr > 0) {
        return;
      }
    }

    throw new Error(
      `No progress after ${MAX_CONSECUTIVE_EMPTY_READS} read() calls`
    );
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
    this.eof = false;
    // this.lastByte = -1;
    // this.lastCharSize = -1;
  }

  /** reads data into p.
   * It returns the number of bytes read into p.
   * The bytes are taken from at most one Read on the underlying Reader,
   * hence n may be less than len(p).
   * To read exactly len(p) bytes, use io.ReadFull(b, p).
   */
  async read(p: Uint8Array): Promise<number | null> {
    let rr: number | null = p.byteLength;
    if (p.byteLength === 0) return rr;

    if (this.r === this.w) {
      if (p.byteLength >= this.buf.byteLength) {
        // Large read, empty buffer.
        // Read directly into p to avoid copy.
        const rr = await this.rd.read(p);
        const nread = rr ?? 0;
        assert(nread >= 0, "negative read");
        // if (rr.nread > 0) {
        //   this.lastByte = p[rr.nread - 1];
        //   this.lastCharSize = -1;
        // }
        return rr;
      }

      // One read.
      // Do not use this.fill, which will loop.
      this.r = 0;
      this.w = 0;
      rr = await this.rd.read(this.buf);
      if (rr === 0 || rr === null) return rr;
      assert(rr >= 0, "negative read");
      this.w += rr;
    }

    // copy as much as we can
    const copied = copyBytes(this.buf.subarray(this.r, this.w), p, 0);
    this.r += copied;
    // this.lastByte = this.buf[this.r - 1];
    // this.lastCharSize = -1;
    return copied;
  }

  /** reads exactly `p.length` bytes into `p`.
   *
   * If successful, `p` is returned.
   *
   * If the end of the underlying stream has been reached, and there are no more
   * bytes available in the buffer, `readFull()` returns `null` instead.
   *
   * An error is thrown if some bytes could be read, but not enough to fill `p`
   * entirely before the underlying stream reported an error or EOF. Any error
   * thrown will have a `partial` property that indicates the slice of the
   * buffer that has been successfully filled with data.
   *
   * Ported from https://golang.org/pkg/io/#ReadFull
   */
  async readFull(p: Uint8Array): Promise<Uint8Array | null> {
    let bytesRead = 0;
    while (bytesRead < p.length) {
      try {
        const rr = await this.read(p.subarray(bytesRead));
        if (rr === null) {
          if (bytesRead === 0) {
            return null;
          } else {
            throw new PartialReadError();
          }
        }
        bytesRead += rr;
      } catch (err) {
        err.partial = p.subarray(0, bytesRead);
        throw err;
      }
    }
    return p;
  }

  /** Returns the next byte [0, 255] or `null`. */
  async readByte(): Promise<number | null> {
    while (this.r === this.w) {
      if (this.eof) return null;
      await this._fill(); // buffer is empty.
    }
    const c = this.buf[this.r];
    this.r++;
    // this.lastByte = c;
    return c;
  }

  /** readString() reads until the first occurrence of delim in the input,
   * returning a string containing the data up to and including the delimiter.
   * If ReadString encounters an error before finding a delimiter,
   * it returns the data read before the error and the error itself
   * (often `null`).
   * ReadString returns err != nil if and only if the returned data does not end
   * in delim.
   * For simple uses, a Scanner may be more convenient.
   */
  async readString(delim: string): Promise<string | null> {
    if (delim.length !== 1) {
      throw new Error("Delimiter should be a single character");
    }
    const buffer = await this.readSlice(delim.charCodeAt(0));
    if (buffer === null) return null;
    return new TextDecoder().decode(buffer);
  }

  /** `readLine()` is a low-level line-reading primitive. Most callers should
   * use `readString('\n')` instead or use a Scanner.
   *
   * `readLine()` tries to return a single line, not including the end-of-line
   * bytes. If the line was too long for the buffer then `more` is set and the
   * beginning of the line is returned. The rest of the line will be returned
   * from future calls. `more` will be false when returning the last fragment
   * of the line. The returned buffer is only valid until the next call to
   * `readLine()`.
   *
   * The text returned from ReadLine does not include the line end ("\r\n" or
   * "\n").
   *
   * When the end of the underlying stream is reached, the final bytes in the
   * stream are returned. No indication or error is given if the input ends
   * without a final line end. When there are no more trailing bytes to read,
   * `readLine()` returns `null`.
   *
   * Calling `unreadByte()` after `readLine()` will always unread the last byte
   * read (possibly a character belonging to the line end) even if that byte is
   * not part of the line returned by `readLine()`.
   */
  async readLine(): Promise<ReadLineResult | null> {
    let line: Uint8Array | null;

    try {
      line = await this.readSlice(LF);
    } catch (err) {
      let { partial } = err;
      assert(
        partial instanceof Uint8Array,
        "bufio: caught error from `readSlice()` without `partial` property"
      );

      // Don't throw if `readSlice()` failed with `BufferFullError`, instead we
      // just return whatever is available and set the `more` flag.
      if (!(err instanceof BufferFullError)) {
        throw err;
      }

      // Handle the case where "\r\n" straddles the buffer.
      if (
        !this.eof &&
        partial.byteLength > 0 &&
        partial[partial.byteLength - 1] === CR
      ) {
        // Put the '\r' back on buf and drop it from line.
        // Let the next call to ReadLine check for "\r\n".
        assert(this.r > 0, "bufio: tried to rewind past start of buffer");
        this.r--;
        partial = partial.subarray(0, partial.byteLength - 1);
      }

      return { line: partial, more: !this.eof };
    }

    if (line === null) {
      return null;
    }

    if (line.byteLength === 0) {
      return { line, more: false };
    }

    if (line[line.byteLength - 1] == LF) {
      let drop = 1;
      if (line.byteLength > 1 && line[line.byteLength - 2] === CR) {
        drop = 2;
      }
      line = line.subarray(0, line.byteLength - drop);
    }
    return { line, more: false };
  }

  /** `readSlice()` reads until the first occurrence of `delim` in the input,
   * returning a slice pointing at the bytes in the buffer. The bytes stop
   * being valid at the next read.
   *
   * If `readSlice()` encounters an error before finding a delimiter, or the
   * buffer fills without finding a delimiter, it throws an error with a
   * `partial` property that contains the entire buffer.
   *
   * If `readSlice()` encounters the end of the underlying stream and there are
   * any bytes left in the buffer, the rest of the buffer is returned. In other
   * words, EOF is always treated as a delimiter. Once the buffer is empty,
   * it returns `null`.
   *
   * Because the data returned from `readSlice()` will be overwritten by the
   * next I/O operation, most clients should use `readString()` instead.
   */
  async readSlice(delim: number): Promise<Uint8Array | null> {
    let s = 0; // search start index
    let slice: Uint8Array | undefined;

    while (true) {
      // Search buffer.
      let i = this.buf.subarray(this.r + s, this.w).indexOf(delim);
      if (i >= 0) {
        i += s;
        slice = this.buf.subarray(this.r, this.r + i + 1);
        this.r += i + 1;
        break;
      }

      // EOF?
      if (this.eof) {
        if (this.r === this.w) {
          return null;
        }
        slice = this.buf.subarray(this.r, this.w);
        this.r = this.w;
        break;
      }

      // Buffer full?
      if (this.buffered() >= this.buf.byteLength) {
        this.r = this.w;
        // #4521 The internal buffer should not be reused across reads because it causes corruption of data.
        const oldbuf = this.buf;
        const newbuf = this.buf.slice(0);
        this.buf = newbuf;
        throw new BufferFullError(oldbuf);
      }

      s = this.w - this.r; // do not rescan area we scanned before

      // Buffer is not full.
      try {
        await this._fill();
      } catch (err) {
        err.partial = slice;
        throw err;
      }
    }

    // Handle last byte, if any.
    // const i = slice.byteLength - 1;
    // if (i >= 0) {
    //   this.lastByte = slice[i];
    //   this.lastCharSize = -1
    // }

    return slice;
  }

  /** `peek()` returns the next `n` bytes without advancing the reader. The
   * bytes stop being valid at the next read call.
   *
   * When the end of the underlying stream is reached, but there are unread
   * bytes left in the buffer, those bytes are returned. If there are no bytes
   * left in the buffer, it returns `null`.
   *
   * If an error is encountered before `n` bytes are available, `peek()` throws
   * an error with the `partial` property set to a slice of the buffer that
   * contains the bytes that were available before the error occurred.
   */
  async peek(n: number): Promise<Uint8Array | null> {
    if (n < 0) {
      throw Error("negative count");
    }

    let avail = this.w - this.r;
    while (avail < n && avail < this.buf.byteLength && !this.eof) {
      try {
        await this._fill();
      } catch (err) {
        err.partial = this.buf.subarray(this.r, this.w);
        throw err;
      }
      avail = this.w - this.r;
    }

    if (avail === 0 && this.eof) {
      return null;
    } else if (avail < n && this.eof) {
      return this.buf.subarray(this.r, this.r + avail);
    } else if (avail < n) {
      throw new BufferFullError(this.buf.subarray(this.r, this.w));
    }

    return this.buf.subarray(this.r, this.r + n);
  }
}

abstract class AbstractBufBase {
  buf!: Uint8Array;
  usedBufferBytes = 0;
  err: Error | null = null;

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

  checkBytesWritten(numBytesWritten: number): void {
    if (numBytesWritten < this.usedBufferBytes) {
      if (numBytesWritten > 0) {
        this.buf.copyWithin(0, numBytesWritten, this.usedBufferBytes);
        this.usedBufferBytes -= numBytesWritten;
      }
      this.err = new Error("Short write");
      throw this.err;
    }
  }
}

/** BufWriter implements buffering for an deno.Writer object.
 * If an error occurs writing to a Writer, no more data will be
 * accepted and all subsequent writes, and flush(), will return the error.
 * After all data has been written, the client should call the
 * flush() method to guarantee all data has been forwarded to
 * the underlying deno.Writer.
 */
export class BufWriter extends AbstractBufBase implements Writer {
  /** return new BufWriter unless writer is BufWriter */
  static create(writer: Writer, size: number = DEFAULT_BUF_SIZE): BufWriter {
    return writer instanceof BufWriter ? writer : new BufWriter(writer, size);
  }

  constructor(private writer: Writer, size: number = DEFAULT_BUF_SIZE) {
    super();
    if (size <= 0) {
      size = DEFAULT_BUF_SIZE;
    }
    this.buf = new Uint8Array(size);
  }

  /** Discards any unflushed buffered data, clears any error, and
   * resets buffer to write its output to w.
   */
  reset(w: Writer): void {
    this.err = null;
    this.usedBufferBytes = 0;
    this.writer = w;
  }

  /** Flush writes any buffered data to the underlying io.Writer. */
  async flush(): Promise<void> {
    if (this.err !== null) throw this.err;
    if (this.usedBufferBytes === 0) return;

    let numBytesWritten = 0;
    try {
      numBytesWritten = await this.writer.write(
        this.buf.subarray(0, this.usedBufferBytes)
      );
    } catch (e) {
      this.err = e;
      throw e;
    }

    this.checkBytesWritten(numBytesWritten);

    this.usedBufferBytes = 0;
  }

  /** Writes the contents of `data` into the buffer.  If the contents won't fully
   * fit into the buffer, those bytes that can are copied into the buffer, the
   * buffer is the flushed to the writer and the remaining bytes are copied into
   * the now empty buffer.
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
          numBytesWritten = await this.writer.write(data);
        } catch (e) {
          this.err = e;
          throw e;
        }
      } else {
        numBytesWritten = copyBytes(data, this.buf, this.usedBufferBytes);
        this.usedBufferBytes += numBytesWritten;
        await this.flush();
      }
      totalBytesWritten += numBytesWritten;
      data = data.subarray(numBytesWritten);
    }

    numBytesWritten = copyBytes(data, this.buf, this.usedBufferBytes);
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
 */
export class BufWriterSync extends AbstractBufBase implements WriterSync {
  /** return new BufWriterSync unless writer is BufWriterSync */
  static create(
    writer: WriterSync,
    size: number = DEFAULT_BUF_SIZE
  ): BufWriterSync {
    return writer instanceof BufWriterSync
      ? writer
      : new BufWriterSync(writer, size);
  }

  constructor(private writer: WriterSync, size: number = DEFAULT_BUF_SIZE) {
    super();
    if (size <= 0) {
      size = DEFAULT_BUF_SIZE;
    }
    this.buf = new Uint8Array(size);
  }

  /** Discards any unflushed buffered data, clears any error, and
   * resets buffer to write its output to w.
   */
  reset(w: WriterSync): void {
    this.err = null;
    this.usedBufferBytes = 0;
    this.writer = w;
  }

  /** Flush writes any buffered data to the underlying io.WriterSync. */
  flush(): void {
    if (this.err !== null) throw this.err;
    if (this.usedBufferBytes === 0) return;

    let numBytesWritten = 0;
    try {
      numBytesWritten = this.writer.writeSync(
        this.buf.subarray(0, this.usedBufferBytes)
      );
    } catch (e) {
      this.err = e;
      throw e;
    }

    this.checkBytesWritten(numBytesWritten);

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
          numBytesWritten = this.writer.writeSync(data);
        } catch (e) {
          this.err = e;
          throw e;
        }
      } else {
        numBytesWritten = copyBytes(data, this.buf, this.usedBufferBytes);
        this.usedBufferBytes += numBytesWritten;
        this.flush();
      }
      totalBytesWritten += numBytesWritten;
      data = data.subarray(numBytesWritten);
    }

    numBytesWritten = copyBytes(data, this.buf, this.usedBufferBytes);
    this.usedBufferBytes += numBytesWritten;
    totalBytesWritten += numBytesWritten;
    return totalBytesWritten;
  }
}

/** Generate longest proper prefix which is also suffix array. */
function createLPS(pat: Uint8Array): Uint8Array {
  const lps = new Uint8Array(pat.length);
  lps[0] = 0;
  let prefixEnd = 0;
  let i = 1;
  while (i < lps.length) {
    if (pat[i] == pat[prefixEnd]) {
      prefixEnd++;
      lps[i] = prefixEnd;
      i++;
    } else if (prefixEnd === 0) {
      lps[i] = 0;
      i++;
    } else {
      prefixEnd = pat[prefixEnd - 1];
    }
  }
  return lps;
}

/** Read delimited bytes from a Reader. */
export async function* readDelim(
  reader: Reader,
  delim: Uint8Array
): AsyncIterableIterator<Uint8Array> {
  // Avoid unicode problems
  const delimLen = delim.length;
  const delimLPS = createLPS(delim);

  let inputBuffer = new Deno.Buffer();
  const inspectArr = new Uint8Array(Math.max(1024, delimLen + 1));

  // Modified KMP
  let inspectIndex = 0;
  let matchIndex = 0;
  while (true) {
    const result = await reader.read(inspectArr);
    if (result === null) {
      // Yield last chunk.
      yield inputBuffer.bytes();
      return;
    }
    if ((result as number) < 0) {
      // Discard all remaining and silently fail.
      return;
    }
    const sliceRead = inspectArr.subarray(0, result as number);
    await Deno.writeAll(inputBuffer, sliceRead);

    let sliceToProcess = inputBuffer.bytes();
    while (inspectIndex < sliceToProcess.length) {
      if (sliceToProcess[inspectIndex] === delim[matchIndex]) {
        inspectIndex++;
        matchIndex++;
        if (matchIndex === delimLen) {
          // Full match
          const matchEnd = inspectIndex - delimLen;
          const readyBytes = sliceToProcess.subarray(0, matchEnd);
          // Copy
          const pendingBytes = sliceToProcess.slice(inspectIndex);
          yield readyBytes;
          // Reset match, different from KMP.
          sliceToProcess = pendingBytes;
          inspectIndex = 0;
          matchIndex = 0;
        }
      } else {
        if (matchIndex === 0) {
          inspectIndex++;
        } else {
          matchIndex = delimLPS[matchIndex - 1];
        }
      }
    }
    // Keep inspectIndex and matchIndex.
    inputBuffer = new Deno.Buffer(sliceToProcess);
  }
}

/** Read delimited strings from a Reader. */
export async function* readStringDelim(
  reader: Reader,
  delim: string
): AsyncIterableIterator<string> {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  for await (const chunk of readDelim(reader, encoder.encode(delim))) {
    yield decoder.decode(chunk);
  }
}

/** Read strings line-by-line from a Reader. */
// eslint-disable-next-line require-await
export async function* readLines(
  reader: Reader
): AsyncIterableIterator<string> {
  yield* readStringDelim(reader, "\n");
}
