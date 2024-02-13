// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/** See the Contributing > Types section in the README for an explanation of this file. */

/** An abstract interface which when implemented provides an interface to read bytes into an array buffer asynchronously. */
export interface Reader {
  /** Reads up to `p.byteLength` bytes into `p`. It resolves to the number of
   * bytes read (`0` < `n` <= `p.byteLength`) and rejects if any error
   * encountered. Even if `read()` resolves to `n` < `p.byteLength`, it may
   * use all of `p` as scratch space during the call. If some data is
   * available but not `p.byteLength` bytes, `read()` conventionally resolves
   * to what is available instead of waiting for more.
   *
   * When `read()` encounters end-of-file condition, it resolves to EOF
   * (`null`).
   *
   * When `read()` encounters an error, it rejects with an error.
   *
   * Callers should always process the `n` > `0` bytes returned before
   * considering the EOF (`null`). Doing so correctly handles I/O errors that
   * happen after reading some bytes and also both of the allowed EOF
   * behaviors.
   *
   * Implementations should not retain a reference to `p`.
   *
   * Use iterateReader() from https://deno.land/std@$STD_VERSION/streams/iterate_reader.ts to turn a Reader into an
   * AsyncIterator.
   */
  read(p: Uint8Array): Promise<number | null>;
}

/** An abstract interface which when implemented provides an interface to read bytes into an array buffer synchronously. */
export interface ReaderSync {
  /** Reads up to `p.byteLength` bytes into `p`. It resolves to the number
   * of bytes read (`0` < `n` <= `p.byteLength`) and rejects if any error
   * encountered. Even if `read()` returns `n` < `p.byteLength`, it may use
   * all of `p` as scratch space during the call. If some data is available
   * but not `p.byteLength` bytes, `read()` conventionally returns what is
   * available instead of waiting for more.
   *
   * When `readSync()` encounters end-of-file condition, it returns EOF
   * (`null`).
   *
   * When `readSync()` encounters an error, it throws with an error.
   *
   * Callers should always process the `n` > `0` bytes returned before
   * considering the EOF (`null`). Doing so correctly handles I/O errors that happen
   * after reading some bytes and also both of the allowed EOF behaviors.
   *
   * Implementations should not retain a reference to `p`.
   *
   * Use iterateReaderSync() from https://deno.land/std@$STD_VERSION/streams/iterate_reader.ts to turn a ReaderSync
   * into an Iterator.
   */
  readSync(p: Uint8Array): number | null;
}

/** An abstract interface which when implemented provides an interface to write bytes from an array buffer to a file/resource asynchronously. */
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
/** An abstract interface which when implemented provides an interface to write bytes from an array buffer to a file/resource synchronously. */
export interface WriterSync {
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

/** An abstract interface which when implemented provides an interface to close files/resources that were previously opened. */
export interface Closer {
  /** Closes the resource, "freeing" the backing file/resource. */
  close(): void;
}
