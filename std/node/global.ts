// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { process as processModule } from "./process.ts";
import { Buffer as bufferModule } from "./buffer.ts";

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

type Buffer = {
  constructor: bufferModule;
  new (): bufferModule;
  alloc(
    size: number,
    fill?: number | string | Uint8Array | bufferModule,
    encoding?: string,
  ): bufferModule;
  allocUnsafe(size: number): bufferModule;
  byteLength(
    string:
      | string
      | bufferModule
      | ArrayBufferView
      | ArrayBuffer
      | SharedArrayBuffer,
    encoding?: string,
  ): number;
  concat(list: bufferModule[] | Uint8Array[], totalLength?: number): bufferModule;
  from(array: number[]): bufferModule;
  from(
    arrayBuffer: ArrayBuffer | SharedArrayBuffer,
    byteOffset?: number,
    length?: number,
  ): bufferModule;
  from(buffer: bufferModule | Uint8Array): bufferModule;
  from(string: string, encoding?: string): bufferModule;
  isBuffer(obj: unknown): obj is bufferModule;
  // deno-lint-ignore no-explicit-any
  isEncoding(encoding: any): boolean;
};

type GlobalType = {
  process: typeof processModule;
  Buffer: Buffer;
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
  var Buffer: Buffer;
}

export {};
