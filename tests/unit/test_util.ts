// Copyright 2018-2025 the Deno authors. MIT license.

import * as colors from "@std/fmt/colors";
import { assert } from "@std/assert";
export { colors };
import { join, resolve } from "@std/path";
export {
  assert,
  assertEquals,
  assertFalse,
  AssertionError,
  assertIsError,
  assertMatch,
  assertNotEquals,
  assertNotStrictEquals,
  assertRejects,
  assertStrictEquals,
  assertStringIncludes,
  assertThrows,
  fail,
  unimplemented,
  unreachable,
} from "@std/assert";
export { delay } from "@std/async/delay";
export { parseArgs } from "@std/cli/parse-args";
import { copy } from "@std/bytes/copy";
import type { Reader, Writer, WriterSync } from "@std/io/types";

export function pathToAbsoluteFileUrl(path: string): URL {
  path = resolve(path);

  return new URL(`file://${Deno.build.os === "windows" ? "/" : ""}${path}`);
}

export function execCode(code: string): Promise<readonly [number, string]> {
  return execCode2(code).finished();
}

export function execCode3(cmd: string, args: string[]) {
  const command = new Deno.Command(cmd, {
    args,
    stdout: "piped",
    stderr: "inherit",
  });

  const child = command.spawn();
  const stdout = child.stdout.pipeThrough(new TextDecoderStream()).getReader();
  let output = "";

  return {
    async waitStdoutText(text: string) {
      while (true) {
        const readData = await stdout.read();
        if (readData.value) {
          output += readData.value;
          if (output.includes(text)) {
            return;
          }
        }
        if (readData.done) {
          throw new Error(`Did not find text '${text}' in stdout.`);
        }
      }
    },
    async finished() {
      while (true) {
        const readData = await stdout.read();
        if (readData.value) {
          output += readData.value;
        }
        if (readData.done) {
          break;
        }
      }
      const status = await child.status;
      return [status.code, output] as const;
    },
  };
}

export function execCode2(code: string) {
  return execCode3(Deno.execPath(), ["eval", code]);
}

export function tmpUnixSocketPath(): string {
  const folder = Deno.makeTempDirSync();
  return join(folder, "socket");
}

export async function curlRequest(args: string[]) {
  const { success, stdout, stderr } = await new Deno.Command("curl", {
    args,
    stdout: "piped",
    stderr: "piped",
  }).output();
  const decoder = new TextDecoder();
  assert(
    success,
    `Failed to cURL ${args}: stdout\n\n${
      decoder.decode(stdout)
    }\n\nstderr:\n\n${decoder.decode(stderr)}`,
  );
  return decoder.decode(stdout);
}

export async function curlRequestWithStdErr(args: string[]) {
  const { success, stdout, stderr } = await new Deno.Command("curl", {
    args,
    stdout: "piped",
    stderr: "piped",
  }).output();
  const decoder = new TextDecoder();
  assert(
    success,
    `Failed to cURL ${args}: stdout\n\n${
      decoder.decode(stdout)
    }\n\nstderr:\n\n${decoder.decode(stderr)}`,
  );
  return [decoder.decode(stdout), decoder.decode(stderr)];
}

const DEFAULT_BUF_SIZE = 4096;
const MIN_BUF_SIZE = 16;
const MAX_CONSECUTIVE_EMPTY_READS = 100;
const CR = "\r".charCodeAt(0);
const LF = "\n".charCodeAt(0);

/**
 * Thrown when a write operation is attempted on a full buffer.
 *
 * @example Usage
 * ```ts
 * import { BufWriter, BufferFullError, Writer } from "@std/io";
 * import { assert, assertEquals } from "@std/assert";
 *
 * const writer: Writer = {
 *   write(p: Uint8Array): Promise<number> {
 *     throw new BufferFullError(p);
 *   }
 * };
 * const bufWriter = new BufWriter(writer);
 * try {
 *   await bufWriter.write(new Uint8Array([1, 2, 3]));
 * } catch (err) {
 *   assert(err instanceof BufferFullError);
 *   assertEquals(err.partial, new Uint8Array([3]));
 * }
 * ```
 */
export class BufferFullError extends Error {
  /**
   * The partially read bytes
   *
   * @example Usage
   * ```ts
   * import { BufferFullError } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const err = new BufferFullError(new Uint8Array(2));
   * assertEquals(err.partial, new Uint8Array(2));
   * ```
   */
  partial: Uint8Array;

  /**
   * Construct a new instance.
   *
   * @param partial The bytes partially read
   */
  constructor(partial: Uint8Array) {
    super("Buffer full");
    this.name = this.constructor.name;
    this.partial = partial;
  }
}

/**
 * Thrown when a read from a stream fails to read the
 * requested number of bytes.
 *
 * @example Usage
 * ```ts
 * import { PartialReadError } from "@std/io";
 * import { assertEquals } from "@std/assert/equals";
 *
 * const err = new PartialReadError(new Uint8Array(2));
 * assertEquals(err.name, "PartialReadError");
 *
 * ```
 */
export class PartialReadError extends Error {
  /**
   * The partially read bytes
   *
   * @example Usage
   * ```ts
   * import { PartialReadError } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const err = new PartialReadError(new Uint8Array(2));
   * assertEquals(err.partial, new Uint8Array(2));
   * ```
   */
  partial: Uint8Array;

  /**
   * Construct a {@linkcode PartialReadError}.
   *
   * @param partial The bytes partially read
   */
  constructor(partial: Uint8Array) {
    super("Encountered UnexpectedEof, data only partially read");
    this.name = this.constructor.name;
    this.partial = partial;
  }
}

/**
 * Result type returned by of {@linkcode BufReader.readLine}.
 */
export interface ReadLineResult {
  /** The line read */
  line: Uint8Array;
  /** `true  if the end of the line has not been reached, `false` otherwise. */
  more: boolean;
}

/**
 * Implements buffering for a {@linkcode Reader} object.
 *
 * @example Usage
 * ```ts
 * import { BufReader, Buffer } from "@std/io";
 * import { assertEquals } from "@std/assert/equals";
 *
 * const encoder = new TextEncoder();
 * const decoder = new TextDecoder();
 *
 * const reader = new BufReader(new Buffer(encoder.encode("hello world")));
 * const buf = new Uint8Array(11);
 * await reader.read(buf);
 * assertEquals(decoder.decode(buf), "hello world");
 * ```
 */
export class BufReader implements Reader {
  #buf!: Uint8Array;
  #rd!: Reader; // Reader provided by caller.
  #r = 0; // buf read position.
  #w = 0; // buf write position.
  #eof = false;

  /**
   * Returns a new {@linkcode BufReader} if `r` is not already one.
   *
   * @example Usage
   * ```ts
   * import { BufReader, Buffer } from "@std/io";
   * import { assert } from "@std/assert/assert";
   *
   * const reader = new Buffer(new TextEncoder().encode("hello world"));
   * const bufReader = BufReader.create(reader);
   * assert(bufReader instanceof BufReader);
   * ```
   *
   * @param r The reader to read from.
   * @param size The size of the buffer.
   * @returns A new {@linkcode BufReader} if `r` is not already one.
   */
  static create(r: Reader, size: number = DEFAULT_BUF_SIZE): BufReader {
    return r instanceof BufReader ? r : new BufReader(r, size);
  }

  /**
   * Constructs a new {@linkcode BufReader} for the given reader and buffer size.
   *
   * @param rd The reader to read from.
   * @param size The size of the buffer.
   */
  constructor(rd: Reader, size: number = DEFAULT_BUF_SIZE) {
    if (size < MIN_BUF_SIZE) {
      size = MIN_BUF_SIZE;
    }
    this.#reset(new Uint8Array(size), rd);
  }

  /**
   * Returns the size of the underlying buffer in bytes.
   *
   * @example Usage
   * ```ts
   * import { BufReader, Buffer } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const reader = new Buffer(new TextEncoder().encode("hello world"));
   * const bufReader = new BufReader(reader);
   *
   * assertEquals(bufReader.size(), 4096);
   * ```
   *
   * @returns The size of the underlying buffer in bytes.
   */
  size(): number {
    return this.#buf.byteLength;
  }

  /**
   * Returns the number of bytes that can be read from the current buffer.
   *
   * @example Usage
   * ```ts
   * import { BufReader, Buffer } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const reader = new Buffer(new TextEncoder().encode("hello world"));
   * const bufReader = new BufReader(reader);
   * await bufReader.read(new Uint8Array(5));
   * assertEquals(bufReader.buffered(), 6);
   * ```
   *
   * @returns Number of bytes that can be read from the buffer
   */
  buffered(): number {
    return this.#w - this.#r;
  }

  // Reads a new chunk into the buffer.
  #fill = async () => {
    // Slide existing data to beginning.
    if (this.#r > 0) {
      this.#buf.copyWithin(0, this.#r, this.#w);
      this.#w -= this.#r;
      this.#r = 0;
    }

    if (this.#w >= this.#buf.byteLength) {
      throw new Error("Buffer full while filling");
    }

    // Read new data: try a limited number of times.
    for (let i = MAX_CONSECUTIVE_EMPTY_READS; i > 0; i--) {
      const rr = await this.#rd.read(this.#buf.subarray(this.#w));
      if (rr === null) {
        this.#eof = true;
        return;
      }
      this.#w += rr;
      if (rr > 0) {
        return;
      }
    }

    throw new Error(
      `No progress after ${MAX_CONSECUTIVE_EMPTY_READS} read() calls`,
    );
  };

  /**
   * Discards any buffered data, resets all state, and switches
   * the buffered reader to read from `r`.
   *
   * @example Usage
   * ```ts
   * import { BufReader, Buffer } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const reader = new Buffer(new TextEncoder().encode("hello world"));
   * const bufReader = new BufReader(reader);
   * await bufReader.read(new Uint8Array(5));
   * bufReader.reset(reader);
   * assertEquals(bufReader.buffered(), 6);
   * ```
   *
   * @param r The reader to read from.
   */
  reset(r: Reader) {
    this.#reset(this.#buf, r);
  }

  #reset = (buf: Uint8Array, rd: Reader) => {
    this.#buf = buf;
    this.#rd = rd;
    this.#eof = false;
    // this.lastByte = -1;
    // this.lastCharSize = -1;
  };

  /**
   * Reads data into `p`.
   *
   * The bytes are taken from at most one `read()` on the underlying `Reader`,
   * hence n may be less than `len(p)`.
   * To read exactly `len(p)` bytes, use `io.ReadFull(b, p)`.
   *
   * @example Usage
   * ```ts
   * import { BufReader, Buffer } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const reader = new Buffer(new TextEncoder().encode("hello world"));
   * const bufReader = new BufReader(reader);
   * const buf = new Uint8Array(5);
   * await bufReader.read(buf);
   * assertEquals(new TextDecoder().decode(buf), "hello");
   * ```
   *
   * @param p The buffer to read data into.
   * @returns The number of bytes read into `p`.
   */
  async read(p: Uint8Array): Promise<number | null> {
    let rr: number | null = p.byteLength;
    if (p.byteLength === 0) return rr;

    if (this.#r === this.#w) {
      if (p.byteLength >= this.#buf.byteLength) {
        // Large read, empty buffer.
        // Read directly into p to avoid copy.
        const rr = await this.#rd.read(p);
        // if (rr.nread > 0) {
        //   this.lastByte = p[rr.nread - 1];
        //   this.lastCharSize = -1;
        // }
        return rr;
      }

      // One read.
      // Do not use this.fill, which will loop.
      this.#r = 0;
      this.#w = 0;
      rr = await this.#rd.read(this.#buf);
      if (rr === 0 || rr === null) return rr;
      this.#w += rr;
    }

    // copy as much as we can
    const copied = copy(this.#buf.subarray(this.#r, this.#w), p, 0);
    this.#r += copied;
    // this.lastByte = this.buf[this.r - 1];
    // this.lastCharSize = -1;
    return copied;
  }

  /**
   * Reads exactly `p.length` bytes into `p`.
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
   *
   * @example Usage
   * ```ts
   * import { BufReader, Buffer } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const reader = new Buffer(new TextEncoder().encode("hello world"));
   * const bufReader = new BufReader(reader);
   * const buf = new Uint8Array(5);
   * await bufReader.readFull(buf);
   * assertEquals(new TextDecoder().decode(buf), "hello");
   * ```
   *
   * @param p The buffer to read data into.
   * @returns The buffer `p` if the read is successful, `null` if the end of the
   * underlying stream has been reached, and there are no more bytes available in the buffer.
   */
  async readFull(p: Uint8Array): Promise<Uint8Array | null> {
    let bytesRead = 0;
    while (bytesRead < p.length) {
      const rr = await this.read(p.subarray(bytesRead));
      if (rr === null) {
        if (bytesRead === 0) {
          return null;
        } else {
          throw new PartialReadError(p.subarray(0, bytesRead));
        }
      }
      bytesRead += rr;
    }
    return p;
  }

  /**
   * Returns the next byte ([0, 255]) or `null`.
   *
   * @example Usage
   * ```ts
   * import { BufReader, Buffer } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const reader = new Buffer(new TextEncoder().encode("hello world"));
   * const bufReader = new BufReader(reader);
   * const byte = await bufReader.readByte();
   * assertEquals(byte, 104);
   * ```
   *
   * @returns The next byte ([0, 255]) or `null`.
   */
  async readByte(): Promise<number | null> {
    while (this.#r === this.#w) {
      if (this.#eof) return null;
      await this.#fill(); // buffer is empty.
    }
    const c = this.#buf[this.#r]!;
    this.#r++;
    // this.lastByte = c;
    return c;
  }

  /**
   * Reads until the first occurrence of delim in the input,
   * returning a string containing the data up to and including the delimiter.
   * If ReadString encounters an error before finding a delimiter,
   * it returns the data read before the error and the error itself
   * (often `null`).
   * ReadString returns err !== null if and only if the returned data does not end
   * in delim.
   *
   * @example Usage
   * ```ts
   * import { BufReader, Buffer } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const reader = new Buffer(new TextEncoder().encode("hello world"));
   * const bufReader = new BufReader(reader);
   * const str = await bufReader.readString(" ");
   * assertEquals(str, "hello ");
   *
   * const str2 = await bufReader.readString(" ");
   * assertEquals(str2, "world");
   * ```
   *
   * @param delim The delimiter to read until.
   * @returns The string containing the data up to and including the delimiter.
   */
  async readString(delim: string): Promise<string | null> {
    if (delim.length !== 1) {
      throw new Error("Delimiter should be a single character");
    }
    const buffer = await this.readSlice(delim.charCodeAt(0));
    if (buffer === null) return null;
    return new TextDecoder().decode(buffer);
  }

  /**
   * A low-level line-reading primitive. Most callers should use
   * `readString('\n')` instead.
   *
   * `readLine()` tries to return a single line, not including the end-of-line
   * bytes. If the line was too long for the buffer then `more` is set and the
   * beginning of the line is returned. The rest of the line will be returned
   * from future calls. `more` will be false when returning the last fragment
   * of the line. The returned buffer is only valid until the next call to
   * `readLine()`.
   *
   * The text returned from this method does not include the line end ("\r\n" or
   * "\n").
   *
   * When the end of the underlying stream is reached, the final bytes in the
   * stream are returned. No indication or error is given if the input ends
   * without a final line end. When there are no more trailing bytes to read,
   * `readLine()` returns `null`.
   *
   * @example Usage
   * ```ts
   * import { BufReader, Buffer } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const reader = new Buffer(new TextEncoder().encode("hello\nworld"));
   * const bufReader = new BufReader(reader);
   * const line1 = await bufReader.readLine();
   * assertEquals(new TextDecoder().decode(line1!.line), "hello");
   * const line2 = await bufReader.readLine();
   * assertEquals(new TextDecoder().decode(line2!.line), "world");
   * ```
   *
   * @returns The line read.
   */
  async readLine(): Promise<ReadLineResult | null> {
    let line: Uint8Array | null = null;

    try {
      line = await this.readSlice(LF);
    } catch (err) {
      // Don't throw if `readSlice()` failed with `BufferFullError`, instead we
      // just return whatever is available and set the `more` flag.
      if (!(err instanceof BufferFullError)) {
        throw err;
      }

      let partial = err.partial;

      // Handle the case where "\r\n" straddles the buffer.
      if (
        !this.#eof && partial &&
        partial.byteLength > 0 &&
        partial[partial.byteLength - 1] === CR
      ) {
        // Put the '\r' back on buf and drop it from line.
        // Let the next call to ReadLine check for "\r\n".
        if (this.#r <= 0) {
          throw new Error("Tried to rewind past start of buffer");
        }
        this.#r--;
        partial = partial.subarray(0, partial.byteLength - 1);
      }

      if (partial) {
        return { line: partial, more: !this.#eof };
      }
    }

    if (line === null) {
      return null;
    }

    if (line.byteLength === 0) {
      return { line, more: false };
    }

    if (line[line.byteLength - 1] === LF) {
      let drop = 1;
      if (line.byteLength > 1 && line[line.byteLength - 2] === CR) {
        drop = 2;
      }
      line = line.subarray(0, line.byteLength - drop);
    }
    return { line, more: false };
  }

  /**
   * Reads until the first occurrence of `delim` in the input,
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
   *
   * @example Usage
   * ```ts
   * import { BufReader, Buffer } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const reader = new Buffer(new TextEncoder().encode("hello world"));
   * const bufReader = new BufReader(reader);
   * const slice = await bufReader.readSlice(0x20);
   * assertEquals(new TextDecoder().decode(slice!), "hello ");
   * ```
   *
   * @param delim The delimiter to read until.
   * @returns A slice pointing at the bytes in the buffer.
   */
  async readSlice(delim: number): Promise<Uint8Array | null> {
    let s = 0; // search start index
    let slice: Uint8Array | undefined;

    while (true) {
      // Search buffer.
      let i = this.#buf.subarray(this.#r + s, this.#w).indexOf(delim);
      if (i >= 0) {
        i += s;
        slice = this.#buf.subarray(this.#r, this.#r + i + 1);
        this.#r += i + 1;
        break;
      }

      // EOF?
      if (this.#eof) {
        if (this.#r === this.#w) {
          return null;
        }
        slice = this.#buf.subarray(this.#r, this.#w);
        this.#r = this.#w;
        break;
      }

      // Buffer full?
      if (this.buffered() >= this.#buf.byteLength) {
        this.#r = this.#w;
        // #4521 The internal buffer should not be reused across reads because it causes corruption of data.
        const oldbuf = this.#buf;
        const newbuf = this.#buf.slice(0);
        this.#buf = newbuf;
        throw new BufferFullError(oldbuf);
      }

      s = this.#w - this.#r; // do not rescan area we scanned before

      // Buffer is not full.
      await this.#fill();
    }

    // Handle last byte, if any.
    // const i = slice.byteLength - 1;
    // if (i >= 0) {
    //   this.lastByte = slice[i];
    //   this.lastCharSize = -1
    // }

    return slice;
  }

  /**
   * Returns the next `n` bytes without advancing the reader. The
   * bytes stop being valid at the next read call.
   *
   * When the end of the underlying stream is reached, but there are unread
   * bytes left in the buffer, those bytes are returned. If there are no bytes
   * left in the buffer, it returns `null`.
   *
   * If an error is encountered before `n` bytes are available, `peek()` throws
   * an error with the `partial` property set to a slice of the buffer that
   * contains the bytes that were available before the error occurred.
   *
   * @example Usage
   * ```ts
   * import { BufReader, Buffer } from "@std/io";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const reader = new Buffer(new TextEncoder().encode("hello world"));
   * const bufReader = new BufReader(reader);
   * const peeked = await bufReader.peek(5);
   * assertEquals(new TextDecoder().decode(peeked!), "hello");
   * ```
   *
   * @param n The number of bytes to peek.
   * @returns The next `n` bytes without advancing the reader.
   */
  async peek(n: number): Promise<Uint8Array | null> {
    if (n < 0) {
      throw new Error("Peek count cannot be negative");
    }

    let avail = this.#w - this.#r;
    while (avail < n && avail < this.#buf.byteLength && !this.#eof) {
      await this.#fill();
      avail = this.#w - this.#r;
    }

    if (avail === 0 && this.#eof) {
      return null;
    } else if (avail < n && this.#eof) {
      return this.#buf.subarray(this.#r, this.#r + avail);
    } else if (avail < n) {
      throw new BufferFullError(this.#buf.subarray(this.#r, this.#w));
    }

    return this.#buf.subarray(this.#r, this.#r + n);
  }
}

/**
 * AbstractBufBase is a base class which other classes can embed to
 * implement the {@inkcode Reader} and {@linkcode Writer} interfaces.
 * It provides basic implementations of those interfaces based on a buffer
 * array.
 *
 * @example Usage
 * ```ts no-assert
 * import { AbstractBufBase } from "@std/io/buf-writer";
 * import { Reader } from "@std/io/types";
 *
 * class MyBufReader extends AbstractBufBase {
 *   constructor(buf: Uint8Array) {
 *     super(buf);
 *   }
 * }
 * ```
 *
 * @internal
 */
export abstract class AbstractBufBase {
  /**
   * The buffer
   *
   * @example Usage
   * ```ts
   * import { AbstractBufBase } from "@std/io/buf-writer";
   * import { assertEquals } from "@std/assert/equals";
   *
   * class MyBuffer extends AbstractBufBase {}
   *
   * const buf = new Uint8Array(1024);
   * const mb = new MyBuffer(buf);
   *
   * assertEquals(mb.buf, buf);
   * ```
   */
  buf: Uint8Array;
  /**
   * The used buffer bytes
   *
   * @example Usage
   * ```ts
   * import { AbstractBufBase } from "@std/io/buf-writer";
   * import { assertEquals } from "@std/assert/equals";
   *
   * class MyBuffer extends AbstractBufBase {}
   *
   * const buf = new Uint8Array(1024);
   * const mb = new MyBuffer(buf);
   *
   * assertEquals(mb.usedBufferBytes, 0);
   * ```
   */
  usedBufferBytes = 0;
  /**
   * The error
   *
   * @example Usage
   * ```ts
   * import { AbstractBufBase } from "@std/io/buf-writer";
   * import { assertEquals } from "@std/assert/equals";
   *
   * class MyBuffer extends AbstractBufBase {}
   *
   * const buf = new Uint8Array(1024);
   * const mb = new MyBuffer(buf);
   *
   * assertEquals(mb.err, null);
   * ```
   */
  err: Error | null = null;

  /**
   * Construct a {@linkcode AbstractBufBase} instance
   *
   * @param buf The buffer to use.
   */
  constructor(buf: Uint8Array) {
    this.buf = buf;
  }

  /**
   * Size returns the size of the underlying buffer in bytes.
   *
   * @example Usage
   * ```ts
   * import { AbstractBufBase } from "@std/io/buf-writer";
   * import { assertEquals } from "@std/assert/equals";
   *
   * class MyBuffer extends AbstractBufBase {}
   *
   * const buf = new Uint8Array(1024);
   * const mb = new MyBuffer(buf);
   *
   * assertEquals(mb.size(), 1024);
   * ```
   *
   * @return the size of the buffer in bytes.
   */
  size(): number {
    return this.buf.byteLength;
  }

  /**
   * Returns how many bytes are unused in the buffer.
   *
   * @example Usage
   * ```ts
   * import { AbstractBufBase } from "@std/io/buf-writer";
   * import { assertEquals } from "@std/assert/equals";
   *
   * class MyBuffer extends AbstractBufBase {}
   *
   * const buf = new Uint8Array(1024);
   * const mb = new MyBuffer(buf);
   *
   * assertEquals(mb.available(), 1024);
   * ```
   *
   * @return the number of bytes that are unused in the buffer.
   */
  available(): number {
    return this.buf.byteLength - this.usedBufferBytes;
  }

  /**
   * buffered returns the number of bytes that have been written into the
   * current buffer.
   *
   * @example Usage
   * ```ts
   * import { AbstractBufBase } from "@std/io/buf-writer";
   * import { assertEquals } from "@std/assert/equals";
   *
   * class MyBuffer extends AbstractBufBase {}
   *
   * const buf = new Uint8Array(1024);
   * const mb = new MyBuffer(buf);
   *
   * assertEquals(mb.buffered(), 0);
   * ```
   *
   * @return the number of bytes that have been written into the current buffer.
   */
  buffered(): number {
    return this.usedBufferBytes;
  }
}

/**
 * `BufWriter` implements buffering for an {@linkcode Writer} object.
 * If an error occurs writing to a Writer, no more data will be
 * accepted and all subsequent writes, and flush(), will return the error.
 * After all data has been written, the client should call the
 * flush() method to guarantee all data has been forwarded to
 * the underlying deno.Writer.
 *
 * @example Usage
 * ```ts
 * import { BufWriter } from "@std/io/buf-writer";
 * import { assertEquals } from "@std/assert/equals";
 *
 * const writer = {
 *   write(p: Uint8Array): Promise<number> {
 *     return Promise.resolve(p.length);
 *   }
 * };
 *
 * const bufWriter = new BufWriter(writer);
 * const data = new Uint8Array(1024);
 *
 * await bufWriter.write(data);
 * await bufWriter.flush();
 *
 * assertEquals(bufWriter.buffered(), 0);
 * ```
 */
export class BufWriter extends AbstractBufBase implements Writer {
  #writer: Writer;

  /**
   * return new BufWriter unless writer is BufWriter
   *
   * @example Usage
   * ```ts
   * import { BufWriter } from "@std/io/buf-writer";
   * import { Writer } from "@std/io/types";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const writer: Writer = {
   *   write(p: Uint8Array): Promise<number> {
   *     return Promise.resolve(p.length);
   *   }
   * };
   *
   * const bufWriter = BufWriter.create(writer);
   * const data = new Uint8Array(1024);
   *
   * await bufWriter.write(data);
   *
   * assertEquals(bufWriter.buffered(), 1024);
   * ```
   *
   * @param writer The writer to wrap.
   * @param size The size of the buffer.
   *
   * @return a new {@linkcode BufWriter} instance.
   */
  static create(writer: Writer, size: number = DEFAULT_BUF_SIZE): BufWriter {
    return writer instanceof BufWriter ? writer : new BufWriter(writer, size);
  }

  /**
   * Construct a new {@linkcode BufWriter}
   *
   * @param writer The writer to wrap.
   * @param size The size of the buffer.
   */
  constructor(writer: Writer, size: number = DEFAULT_BUF_SIZE) {
    super(new Uint8Array(size <= 0 ? DEFAULT_BUF_SIZE : size));
    this.#writer = writer;
  }

  /**
   * Discards any unflushed buffered data, clears any error, and
   * resets buffer to write its output to w.
   *
   * @example Usage
   * ```ts
   * import { BufWriter } from "@std/io/buf-writer";
   * import { Writer } from "@std/io/types";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const writer: Writer = {
   *   write(p: Uint8Array): Promise<number> {
   *     return Promise.resolve(p.length);
   *   }
   * };
   *
   * const bufWriter = new BufWriter(writer);
   * const data = new Uint8Array(1024);
   *
   * await bufWriter.write(data);
   *
   * assertEquals(bufWriter.buffered(), 1024);
   *
   * bufWriter.reset(writer);
   *
   * assertEquals(bufWriter.buffered(), 0);
   * ```
   *
   * @param w The writer to write to.
   */
  reset(w: Writer) {
    this.err = null;
    this.usedBufferBytes = 0;
    this.#writer = w;
  }

  /**
   * Flush writes any buffered data to the underlying io.Writer.
   *
   * @example Usage
   * ```ts
   * import { BufWriter } from "@std/io/buf-writer";
   * import { Writer } from "@std/io/types";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const writer: Writer = {
   *   write(p: Uint8Array): Promise<number> {
   *     return Promise.resolve(p.length);
   *   }
   * };
   *
   * const bufWriter = new BufWriter(writer);
   * const data = new Uint8Array(1024);
   *
   * await bufWriter.write(data);
   * await bufWriter.flush();
   *
   * assertEquals(bufWriter.buffered(), 0);
   * ```
   */
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

  /**
   * Writes the contents of `data` into the buffer. If the contents won't fully
   * fit into the buffer, those bytes that are copied into the buffer will be flushed
   * to the writer and the remaining bytes are then copied into the now empty buffer.
   *
   * @example Usage
   * ```ts
   * import { BufWriter } from "@std/io/buf-writer";
   * import { Writer } from "@std/io/types";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const writer: Writer = {
   *   write(p: Uint8Array): Promise<number> {
   *     return Promise.resolve(p.length);
   *   }
   * };
   *
   * const bufWriter = new BufWriter(writer);
   * const data = new Uint8Array(1024);
   *
   * await bufWriter.write(data);
   *
   * assertEquals(bufWriter.buffered(), 1024);
   * ```
   *
   * @param data The data to write to the buffer.
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

/**
 * BufWriterSync implements buffering for a deno.WriterSync object.
 * If an error occurs writing to a WriterSync, no more data will be
 * accepted and all subsequent writes, and flush(), will return the error.
 * After all data has been written, the client should call the
 * flush() method to guarantee all data has been forwarded to
 * the underlying deno.WriterSync.
 *
 * @example Usage
 * ```ts
 * import { BufWriterSync } from "@std/io/buf-writer";
 * import { assertEquals } from "@std/assert/equals";
 *
 * const writer = {
 *   writeSync(p: Uint8Array): number {
 *     return p.length;
 *   }
 * };
 *
 * const bufWriter = new BufWriterSync(writer);
 * const data = new Uint8Array(1024);
 *
 * bufWriter.writeSync(data);
 * bufWriter.flush();
 *
 * assertEquals(bufWriter.buffered(), 0);
 * ```
 */
export class BufWriterSync extends AbstractBufBase implements WriterSync {
  #writer: WriterSync;

  /**
   * return new BufWriterSync unless writer is BufWriterSync
   *
   * @example Usage
   * ```ts
   * import { BufWriterSync } from "@std/io/buf-writer";
   * import { WriterSync } from "@std/io/types";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const writer: WriterSync = {
   *   writeSync(p: Uint8Array): number {
   *     return p.length;
   *   }
   * };
   *
   * const bufWriter = BufWriterSync.create(writer);
   * const data = new Uint8Array(1024);
   * bufWriter.writeSync(data);
   * bufWriter.flush();
   *
   * assertEquals(bufWriter.buffered(), 0);
   * ```
   *
   * @param writer The writer to wrap.
   * @param size The size of the buffer.
   * @returns a new {@linkcode BufWriterSync} instance.
   */
  static create(
    writer: WriterSync,
    size: number = DEFAULT_BUF_SIZE,
  ): BufWriterSync {
    return writer instanceof BufWriterSync
      ? writer
      : new BufWriterSync(writer, size);
  }

  /**
   * Construct a new {@linkcode BufWriterSync}
   *
   * @param writer The writer to wrap.
   * @param size The size of the buffer.
   */
  constructor(writer: WriterSync, size: number = DEFAULT_BUF_SIZE) {
    super(new Uint8Array(size <= 0 ? DEFAULT_BUF_SIZE : size));
    this.#writer = writer;
  }

  /**
   * Discards any unflushed buffered data, clears any error, and
   * resets buffer to write its output to w.
   *
   * @example Usage
   * ```ts
   * import { BufWriterSync } from "@std/io/buf-writer";
   * import { WriterSync } from "@std/io/types";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const writer: WriterSync = {
   *   writeSync(p: Uint8Array): number {
   *     return p.length;
   *   }
   * };
   *
   * const bufWriter = new BufWriterSync(writer);
   * const data = new Uint8Array(1024);
   *
   * bufWriter.writeSync(data);
   * bufWriter.flush();
   *
   * assertEquals(bufWriter.buffered(), 0);
   * ```
   *
   * @param w The writer to write to.
   */
  reset(w: WriterSync) {
    this.err = null;
    this.usedBufferBytes = 0;
    this.#writer = w;
  }

  /**
   * Flush writes any buffered data to the underlying io.WriterSync.
   *
   * @example Usage
   * ```ts
   * import { BufWriterSync } from "@std/io/buf-writer";
   * import { WriterSync } from "@std/io/types";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const writer: WriterSync = {
   *   writeSync(p: Uint8Array): number {
   *     return p.length;
   *   }
   * };
   *
   * const bufWriter = new BufWriterSync(writer);
   * const data = new Uint8Array(1024);
   *
   * bufWriter.writeSync(data);
   * bufWriter.flush();
   *
   * assertEquals(bufWriter.buffered(), 0);
   * ```
   */
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
   * @example Usage
   * ```ts
   * import { BufWriterSync } from "@std/io/buf-writer";
   * import { WriterSync } from "@std/io/types";
   * import { assertEquals } from "@std/assert/equals";
   *
   * const writer: WriterSync = {
   *   writeSync(p: Uint8Array): number {
   *     return p.length;
   *   }
   * };
   *
   * const bufWriter = new BufWriterSync(writer);
   * const data = new Uint8Array(1024);
   *
   * bufWriter.writeSync(data);
   * bufWriter.flush();
   *
   * assertEquals(bufWriter.buffered(), 0);
   * ```
   *
   * @param data The data to write to the buffer.
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
