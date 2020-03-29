// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Interfaces 100% copied from Go.
// Documentation liberally lifted from them too.
// Thank you! We love Go!

export const EOF: unique symbol = Symbol("EOF");
export type EOF = typeof EOF;

// Seek whence values.
// https://golang.org/pkg/io/#pkg-constants
export enum SeekMode {
  SEEK_START = 0,
  SEEK_CURRENT = 1,
  SEEK_END = 2,
}

// Reader is the interface that wraps the basic read() method.
// https://golang.org/pkg/io/#Reader
export interface Reader {
  read(p: Uint8Array): Promise<number | EOF>;
}

export interface SyncReader {
  readSync(p: Uint8Array): number | EOF;
}

// Writer is the interface that wraps the basic write() method.
// https://golang.org/pkg/io/#Writer
export interface Writer {
  write(p: Uint8Array): Promise<number>;
}

export interface SyncWriter {
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
  seek(offset: number, whence: SeekMode): Promise<number>;
}

export interface SyncSeeker {
  seekSync(offset: number, whence: SeekMode): number;
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
        done: false,
      };
    },
  };
}
