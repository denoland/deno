// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { encode } from "../encoding/utf8.ts";

/** Reader utility for strings */
export class StringReader extends Deno.Reader {
  private offs = 0;
  private buf = new Uint8Array(encode(this.s));

  constructor(private readonly s: string) {
    super();
  }

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
export class MultiReader extends Deno.Reader {
  private readonly readers: Deno.Reader[];
  private currentIndex = 0;

  constructor(...readers: Deno.Reader[]) {
    super();
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
