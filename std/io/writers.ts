// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
type Writer = Deno.Writer;
import { decode, encode } from "../strings/mod.ts";

/** Writer utility for buffering string chunks */
export class StringWriter implements Writer {
  private chunks: Uint8Array[] = [];
  private byteLength = 0;
  private cache: string | undefined;

  constructor(private base: string = "") {
    const c = encode(base);
    this.chunks.push(c);
    this.byteLength += c.byteLength;
  }

  write(p: Uint8Array): Promise<number> {
    this.chunks.push(p);
    this.byteLength += p.byteLength;
    this.cache = undefined;
    return Promise.resolve(p.byteLength);
  }

  toString(): string {
    if (this.cache) {
      return this.cache;
    }
    const buf = new Uint8Array(this.byteLength);
    let offs = 0;
    for (const chunk of this.chunks) {
      buf.set(chunk, offs);
      offs += chunk.byteLength;
    }
    this.cache = decode(buf);
    return this.cache;
  }
}
