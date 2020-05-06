// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { setFunctionName } from "./internals.ts";
import { customInspect } from "../console.ts";

export class CountQueuingStrategyImpl implements CountQueuingStrategy {
  highWaterMark: number;

  constructor({ highWaterMark }: { highWaterMark: number }) {
    this.highWaterMark = highWaterMark;
  }

  size(): 1 {
    return 1;
  }

  [customInspect](): string {
    return `${this.constructor.name} { highWaterMark: ${String(
      this.highWaterMark
    )}, size: f }`;
  }
}

Object.defineProperty(CountQueuingStrategyImpl.prototype, "size", {
  enumerable: true,
});

setFunctionName(CountQueuingStrategyImpl, "CountQueuingStrategy");

export class ByteLengthQueuingStrategyImpl
  implements ByteLengthQueuingStrategy {
  highWaterMark: number;

  constructor({ highWaterMark }: { highWaterMark: number }) {
    this.highWaterMark = highWaterMark;
  }

  size(chunk: ArrayBufferView): number {
    return chunk.byteLength;
  }

  [customInspect](): string {
    return `${this.constructor.name} { highWaterMark: ${String(
      this.highWaterMark
    )}, size: f }`;
  }
}

Object.defineProperty(ByteLengthQueuingStrategyImpl.prototype, "size", {
  enumerable: true,
});

setFunctionName(CountQueuingStrategyImpl, "CountQueuingStrategy");
