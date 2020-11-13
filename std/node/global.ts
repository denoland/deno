// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { process as processModule } from "./process.ts";
import { Buffer as bufferModule } from "./buffer.ts";
import { Buffer } from "./_buffer.ts";

Object.defineProperty(globalThis, "global", {
  value: globalThis,
  writable: false,
  enumerable: false,
  configurable: true,
});

Object.defineProperty(globalThis, "process", {
  value: processModule,
  enumerable: false,
  writable: true,
  configurable: true,
});

Object.defineProperty(globalThis, "Buffer", {
  value: bufferModule,
  enumerable: false,
  writable: true,
  configurable: true,
});

type GlobalType = {
  process: typeof processModule;
  Buffer: typeof bufferModule;
};

declare global {
  interface Window {
    global: GlobalType;
  }

  interface globalThis {
    global: GlobalType;
  }

  var global: GlobalType;
  var process: typeof processModule;
  // It's necessary to define the static properties of Buffer
  // otherwise they won't be recognized in the Buffer type
  var Buffer: {
    constructor: Buffer;
    new (): Buffer;
    alloc(
      size: number,
      fill?: number | string | Uint8Array | Buffer,
      encoding?: string,
    ): Buffer;
    allocUnsafe(size: number): Buffer;
    byteLength(
      string:
        | string
        | Buffer
        | ArrayBufferView
        | ArrayBuffer
        | SharedArrayBuffer,
      encoding?: string,
    ): number;
    concat(list: Buffer[] | Uint8Array[], totalLength?: number): Buffer;
    from(array: number[]): Buffer;
    from(
      arrayBuffer: ArrayBuffer | SharedArrayBuffer,
      byteOffset?: number,
      length?: number,
    ): Buffer;
    from(buffer: Buffer | Uint8Array): Buffer;
    from(string: string, encoding?: string): Buffer;
    isBuffer(obj: unknown): obj is Buffer;
    // deno-lint-ignore no-explicit-any
    isEncoding(encoding: any): boolean;
  };
}

export {};
