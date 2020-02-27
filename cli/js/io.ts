// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Interfaces 100% copied from Go.
// Documentation liberally lifted from them too.
// Thank you! We love Go!

export const EOF: unique symbol = Symbol("EOF");
export type EOF = typeof EOF;

// Seek whence values.
// https://golang.org/pkg/io/#pkg-constants
/** **UNSTABLE**: might remove `"SEEK_"` prefix. Might not use all-caps. */
export enum SeekMode {
  SEEK_START = 0,
  SEEK_CURRENT = 1,
  SEEK_END = 2
}

// Reader is the interface that wraps the basic read() method.
// https://golang.org/pkg/io/#Reader
/** **UNSTABLE**: might make `Reader` into iterator of some sort. */
export interface Reader {
  /** Reads up to `p.byteLength` bytes into `p`. It resolves to the number of
   * bytes read (`0` < `n` <= `p.byteLength`) and rejects if any error
   * encountered. Even if `read()` resolves to `n` < `p.byteLength`, it may
   * use all of `p` as scratch space during the call. If some data is
   * available but not `p.byteLength` bytes, `read()` conventionally resolves
   * to what is available instead of waiting for more.
   *
   * When `read()` encounters end-of-file condition, it resolves to
   * `Deno.EOF` symbol.
   *
   * When `read()` encounters an error, it rejects with an error.
   *
   * Callers should always process the `n` > `0` bytes returned before
   * considering the `EOF`. Doing so correctly handles I/O errors that happen
   * after reading some bytes and also both of the allowed EOF behaviors.
   *
   * Implementations should not retain a reference to `p`.
   */
  read(p: Uint8Array): Promise<number | EOF>;
}

export interface SyncReader {
  /** Reads up to `p.byteLength` bytes into `p`. It resolves to the number
   * of bytes read (`0` < `n` <= `p.byteLength`) and rejects if any error
   * encountered. Even if `read()` returns `n` < `p.byteLength`, it may use
   * all of `p` as scratch space during the call. If some data is available
   * but not `p.byteLength` bytes, `read()` conventionally returns what is
   * available instead of waiting for more.
   *
   * When `readSync()` encounters end-of-file condition, it returns `Deno.EOF`
   * symbol.
   *
   * When `readSync()` encounters an error, it throws with an error.
   *
   * Callers should always process the `n` > `0` bytes returned before
   * considering the `EOF`. Doing so correctly handles I/O errors that happen
   * after reading some bytes and also both of the allowed EOF behaviors.
   *
   * Implementations should not retain a reference to `p`.
   */
  readSync(p: Uint8Array): number | EOF;
}

// Writer is the interface that wraps the basic write() method.
// https://golang.org/pkg/io/#Writer
export interface Writer {
  /** Writes `p.byteLength` bytes from `p` to the underlying data stream. It
   * resolves to the number of bytes written from `p` (`0` <= `n` <=
   * `p.byteLength`) or reject with the error encountered that caused the
   * write to stop early. `write()` must reject with a non-null error if
   * would resolve to `n` < `p.byteLength`. `write()` must not modify the
   * slice data, even temporarily.
   *
   * Implementations should not retain a reference to `p`.
   */
  write(p: Uint8Array): Promise<number>;
}

export interface SyncWriter {
  /** Writes `p.byteLength` bytes from `p` to the underlying data
   * stream. It returns the number of bytes written from `p` (`0` <= `n`
   * <= `p.byteLength`) and any error encountered that caused the write to
   * stop early. `writeSync()` must throw a non-null error if it returns `n` <
   * `p.byteLength`. `writeSync()` must not modify the slice data, even
   * temporarily.
   *
   * Implementations should not retain a reference to `p`.
   */
  writeSync(p: Uint8Array): number;
}

// https://golang.org/pkg/io/#Closer
export interface Closer {
  // The behavior of Close after the first call is undefined. Specific
  // implementations may document their own behavior.
  close(): void;
}

// https://golang.org/pkg/io/#Seeker
export interface Seeker {
  /** Seek sets the offset for the next `read()` or `write()` to offset,
   * interpreted according to `whence`: `SEEK_START` means relative to the
   * start of the file, `SEEK_CURRENT` means relative to the current offset,
   * and `SEEK_END` means relative to the end.
   *
   * Seeking to an offset before the start of the file is an error. Seeking to
   * any positive offset is legal, but the behavior of subsequent I/O
   * operations on the underlying object is implementation-dependent.
   */
  seek(offset: number, whence: SeekMode): Promise<void>;
}

export interface SyncSeeker {
  /** Seek sets the offset for the next `readSync()` or `writeSync()` to
   * offset, interpreted according to `whence`: `SEEK_START` means relative
   * to the start of the file, `SEEK_CURRENT` means relative to the current
   * offset, and `SEEK_END` means relative to the end.
   *
   * Seeking to an offset before the start of the file is an error. Seeking to
   * any positive offset is legal, but the behavior of subsequent I/O
   * operations on the underlying object is implementation-dependent.
   */
  seekSync(offset: number, whence: SeekMode): void;
}

// https://golang.org/pkg/io/#ReadCloser
export interface ReadCloser extends Reader, Closer {}

// https://golang.org/pkg/io/#WriteCloser
export interface WriteCloser extends Writer, Closer {}

// https://golang.org/pkg/io/#ReadSeeker
export interface ReadSeeker extends Reader, Seeker {}

// https://golang.org/pkg/io/#WriteSeeker
export interface WriteSeeker extends Writer, Seeker {}

// https://golang.org/pkg/io/#ReadWriteCloser
export interface ReadWriteCloser extends Reader, Writer, Closer {}

// https://golang.org/pkg/io/#ReadWriteSeeker
export interface ReadWriteSeeker extends Reader, Writer, Seeker {}

/** Copies from `src` to `dst` until either `EOF` is reached on `src` or an
 * error occurs. It resolves to the number of bytes copied or rejects with
 * the first error encountered while copying.
 *
 * Because `copy()` is defined to read from `src` until `EOF`, it does not
 * treat an `EOF` from `read()` as an error to be reported.
 */
// https://golang.org/pkg/io/#Copy
export async function copy(dst: Writer, src: Reader): Promise<number> {
  let n = 0;
  const b = new Uint8Array(32 * 1024);
  let gotEOF = false;
  while (gotEOF === false) {
    const result = await src.read(b);
    if (result === EOF) {
      gotEOF = true;
    } else {
      n += await dst.write(b.subarray(0, result));
    }
  }
  return n;
}

/** Turns `r` into async iterator.
 *
 *      for await (const chunk of toAsyncIterator(reader)) {
 *        console.log(chunk);
 *      }
 */
export function toAsyncIterator(r: Reader): AsyncIterableIterator<Uint8Array> {
  const b = new Uint8Array(1024);
  return {
    [Symbol.asyncIterator](): AsyncIterableIterator<Uint8Array> {
      return this;
    },

    async next(): Promise<IteratorResult<Uint8Array>> {
      const result = await r.read(b);
      if (result === EOF) {
        return { value: new Uint8Array(), done: true };
      }

      return {
        value: b.subarray(0, result),
        done: false
      };
    }
  };
}
