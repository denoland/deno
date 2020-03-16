// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

/* eslint-disable @typescript-eslint/no-explicit-any */
// TODO reenable this lint here

import { QueuingStrategy } from "../dom_types.ts";

export class ByteLengthQueuingStrategy
  implements QueuingStrategy<ArrayBufferView> {
  highWaterMark: number;

  constructor(options: { highWaterMark: number }) {
    this.highWaterMark = options.highWaterMark;
  }

  size(chunk: ArrayBufferView): number {
    return chunk.byteLength;
  }
}

export class CountQueuingStrategy implements QueuingStrategy<any> {
  highWaterMark: number;

  constructor(options: { highWaterMark: number }) {
    this.highWaterMark = options.highWaterMark;
  }

  size(): number {
    return 1;
  }
}
