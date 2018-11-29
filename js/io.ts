// Interfaces 100% copied from Go.
// Documentation liberally lifted from them too.
// Thank you! We love Go!

// The bytes read during an I/O call and a boolean indicating EOF.
export interface ReadResult {
  nread: number;
  eof: boolean;
}

// Reader is the interface that wraps the basic read() method.
// https://golang.org/pkg/io/#Reader
export interface Reader {
  /** Reads up to p.byteLength bytes into `p`. It resolves to the number
   * of bytes read (`0` <= `n` <= `p.byteLength`) and any error encountered.
   * Even if `read()` returns `n` < `p.byteLength`, it may use all of `p` as
   * scratch space during the call. If some data is available but not
   * `p.byteLength` bytes, `read()` conventionally returns what is available
   * instead of waiting for more.
   *
   * When `read()` encounters an error or end-of-file condition after
   * successfully reading `n` > `0` bytes, it returns the number of bytes read.
   * It may return the (non-nil) error from the same call or return the error
   * (and `n` == `0`) from a subsequent call. An instance of this general case
   * is that a `Reader` returning a non-zero number of bytes at the end of the
   * input stream may return either `err` == `EOF` or `err` == `null`. The next
   * `read()` should return `0`, `EOF`.
   *
   * Callers should always process the `n` > `0` bytes returned before
   * considering the `EOF`. Doing so correctly handles I/O errors that happen
   * after reading some bytes and also both of the allowed `EOF` behaviors.
   *
   * Implementations of `read()` are discouraged from returning a zero byte
   * count with a `null` error, except when `p.byteLength` == `0`. Callers
   * should treat a return of `0` and `null` as indicating that nothing
   * happened; in particular it does not indicate `EOF`.
   *
   * Implementations must not retain `p`.
   */
  read(p: Uint8Array): Promise<ReadResult>;
}

// Writer is the interface that wraps the basic write() method.
// https://golang.org/pkg/io/#Writer
export interface Writer {
  /** Writes `p.byteLength` bytes from `p` to the underlying data
   * stream. It resolves to the number of bytes written from `p` (`0` <= `n` <=
   * `p.byteLength`) and any error encountered that caused the write to stop
   * early. `write()` must return a non-null error if it returns `n` <
   * `p.byteLength`. write() must not modify the slice data, even temporarily.
   *
   * Implementations must not retain `p`.
   */
  write(p: Uint8Array): Promise<number>;
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
   * interpreted according to `whence`: `SeekStart` means relative to the start
   * of the file, `SeekCurrent` means relative to the current offset, and
   * `SeekEnd` means relative to the end. Seek returns the new offset relative
   * to the start of the file and an error, if any.
   *
   * Seeking to an offset before the start of the file is an error. Seeking to
   * any positive offset is legal, but the behavior of subsequent I/O operations
   * on the underlying object is implementation-dependent.
   */
  seek(offset: number, whence: number): Promise<void>;
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

/** Copies from `src` to `dst` until either `EOF` is reached on `src`
 * or an error occurs. It returns the number of bytes copied and the first
 * error encountered while copying, if any.
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
    if (result.eof) {
      gotEOF = true;
    }
    n += await dst.write(b.subarray(0, result.nread));
  }
  return n;
}

/** Turns `r` into async iterator.
 *
 *      for await (const chunk of readerIterator(reader)) {
 *          console.log(chunk)
 *      }
 */
export function toAsyncIterator(r: Reader): AsyncIterableIterator<Uint8Array> {
  const b = new Uint8Array(1024);

  return {
    [Symbol.asyncIterator]() {
      return this;
    },

    async next(): Promise<IteratorResult<Uint8Array>> {
      const result = await r.read(b);
      return {
        value: b.subarray(0, result.nread),
        done: result.eof
      };
    }
  };
}
