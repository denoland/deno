// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { encode } from "../strings/mod.ts";

/** @deprecated Use stringReader() */
export class StringReader implements Deno.Reader {
  #r: Deno.Reader;
  constructor(private readonly s: string) {
    this.#r = stringReader(s);
  }
  read(p: Uint8Array): Promise<number | Deno.EOF> {
    return this.#r.read(p);
  }
}

/** @deprecated use multiReader() */
export class MultiReader implements Deno.Reader {
  #r: Deno.Reader;
  constructor(...readers: Deno.Reader[]) {
    this.#r = multiReader(...readers);
  }
  read(p: Uint8Array): Promise<number | Deno.EOF> {
    return this.#r.read(p);
  }
}

/** Reader utility for combining multiple readers */
export function multiReader(...readers: Deno.Reader[]): Deno.Reader {
  let currentIndex = 0;
  async function read(p: Uint8Array): Promise<number | Deno.EOF> {
    const r = readers[currentIndex];
    if (!r) return Deno.EOF;
    const result = await r.read(p);
    if (result === Deno.EOF) {
      currentIndex++;
      return read(p);
    }
    return result;
  }
  return { read };
}

export function stringReader(s: string): Deno.Reader {
  return bytesReader(encode(s));
}

export function bytesReader(buf: Uint8Array): Deno.Reader {
  let offs = 0;
  function read(p: Uint8Array): Promise<number | Deno.EOF> {
    try {
      const n = Math.min(p.byteLength, buf.byteLength - offs);
      p.set(buf.subarray(offs, offs + n));
      offs += n;
      if (n === 0) {
        return Promise.resolve(Deno.EOF);
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
    read(_: Uint8Array): Promise<number | Deno.EOF> {
      return Promise.resolve(Deno.EOF);
    },
  };
}
