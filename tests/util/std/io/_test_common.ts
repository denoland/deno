// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import type { Reader } from "../types.d.ts";

export const MIN_READ_BUFFER_SIZE = 16;
export const bufsizes: number[] = [
  0,
  MIN_READ_BUFFER_SIZE,
  23,
  32,
  46,
  64,
  93,
  128,
  1024,
  4096,
];

export class BinaryReader implements Reader {
  index = 0;

  constructor(private bytes: Uint8Array = new Uint8Array(0)) {}

  read(p: Uint8Array): Promise<number | null> {
    p.set(this.bytes.subarray(this.index, p.byteLength));
    this.index += p.byteLength;
    return Promise.resolve(p.byteLength);
  }
}
