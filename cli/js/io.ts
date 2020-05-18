// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Interfaces 100% copied from Go.
// Documentation liberally lifted from them too.
// Thank you! We love Go!

const DEFAULT_BUFFER_SIZE = 32 * 1024;

// Seek whence values.
// https://golang.org/pkg/io/#pkg-constants
export enum SeekMode {
  Start = 0,
  Current = 1,
  End = 2,
}

// Reader is the interface that wraps the basic read() method.
// https://golang.org/pkg/io/#Reader
export interface Reader {
  read(p: Uint8Array): Promise<number | null>;
}

export interface ReaderSync {
  readSync(p: Uint8Array): number | null;
}

// Writer is the interface that wraps the basic write() method.
// https://golang.org/pkg/io/#Writer
export interface Writer {
  write(p: Uint8Array): Promise<number>;
}

export interface WriterSync {
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

export interface SeekerSync {
  seekSync(offset: number, whence: SeekMode): number;
}

export async function copy(
  src: Reader,
  dst: Writer,
  options?: {
    bufSize?: number;
  }
): Promise<number> {
  let n = 0;
  const bufSize = options?.bufSize ?? DEFAULT_BUFFER_SIZE;
  const b = new Uint8Array(bufSize);
  let gotEOF = false;
  while (gotEOF === false) {
    const result = await src.read(b);
    if (result === null) {
      gotEOF = true;
    } else {
      let nwritten = 0;
      while (nwritten < result) {
        nwritten += await dst.write(b.subarray(nwritten, result));
      }
      n += nwritten;
    }
  }
  return n;
}

export async function* iter(
  r: Reader,
  options?: {
    bufSize?: number;
  }
): AsyncIterableIterator<Uint8Array> {
  const bufSize = options?.bufSize ?? DEFAULT_BUFFER_SIZE;
  const b = new Uint8Array(bufSize);
  while (true) {
    const result = await r.read(b);
    if (result === null) {
      break;
    }

    yield b.subarray(0, result);
  }
}

export function* iterSync(
  r: ReaderSync,
  options?: {
    bufSize?: number;
  }
): IterableIterator<Uint8Array> {
  const bufSize = options?.bufSize ?? DEFAULT_BUFFER_SIZE;
  const b = new Uint8Array(bufSize);
  while (true) {
    const result = r.readSync(b);
    if (result === null) {
      break;
    }

    yield b.subarray(0, result);
  }
}
