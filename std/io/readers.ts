// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
type Reader = Deno.Reader;
import { encode } from "../strings/mod.ts";

/** Reader utility for strings */
export class StringReader implements Reader {
  private offs = 0;
  private buf = new Uint8Array(encode(this.s));

  constructor(private readonly s: string) {}

  read(p: Uint8Array): Promise<number | Deno.EOF> {
    const n = Math.min(p.byteLength, this.buf.byteLength - this.offs);
    p.set(this.buf.slice(this.offs, this.offs + n));
    this.offs += n;
    if (n === 0) {
      return Promise.resolve(Deno.EOF);
    }
    return Promise.resolve(n);
  }
}

/** Reader utility for combining multiple readers */
export class MultiReader implements Reader {
  private readonly readers: Reader[];
  private currentIndex = 0;

  constructor(...readers: Reader[]) {
    this.readers = readers;
  }

  async read(p: Uint8Array): Promise<number | Deno.EOF> {
    const r = this.readers[this.currentIndex];
    if (!r) return Deno.EOF;
    const result = await r.read(p);
    if (result === Deno.EOF) {
      this.currentIndex++;
      return 0;
    }
    return result;
  }
}
