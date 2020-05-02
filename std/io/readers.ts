// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { encode } from "../encoding/utf8.ts";

/** Reader utility for strings */
export class StringReader implements Deno.Reader {
  private offs = 0;
  private buf = new Uint8Array(encode(this.s));

  constructor(private readonly s: string) {}

  read(p: Uint8Array): Promise<number | null> {
    const n = Math.min(p.byteLength, this.buf.byteLength - this.offs);
    p.set(this.buf.slice(this.offs, this.offs + n));
    this.offs += n;
    if (n === 0) {
      return Promise.resolve(null);
    }
    return Promise.resolve(n);
  }
}

export class MultiReader implements Deno.Reader {
  currentIndex = 0;
  readers: Deno.Reader[];
  constructor(...readers: Deno.Reader[]) {
    this.readers = readers;
  }

  async read(p: Uint8Array): Promise<number | null> {
    const r = this.readers[this.currentIndex];
    if (!r) return null;
    const result = await r.read(p);
    if (result === null) {
      this.currentIndex++;
      return 0;
    }
    return result;
  }
}

export function bytesReader(buf: Uint8Array): Deno.Reader {
  let offs = 0;
  function read(p: Uint8Array): Promise<number | null> {
    try {
      const n = Math.min(p.byteLength, buf.byteLength - offs);
      p.set(buf.subarray(offs, offs + n));
      offs += n;
      if (n === 0) {
        return Promise.resolve(null);
      }
      return Promise.resolve(n);
    } catch (e) {
      return Promise.reject(e);
    }
  }
  return { read };
}

/** Reader that returns EOF everytime */
export function emptyReader(): Deno.Reader {
  return {
    read(_: Uint8Array): Promise<number | null> {
      return Promise.resolve(null);
    },
  };
}
