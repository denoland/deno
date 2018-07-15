// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

//import { debug } from "./main";
const debug = true;

import { TypedArray } from "./types";

// Internal logging for deno. Use the "debug" variable above to control
// output.
// tslint:disable-next-line:no-any
export function log(...args: any[]): void {
  if (debug) {
    console.log(...args);
  }
}

export function assert(cond: boolean, msg = "") {
  if (!cond) {
    throw Error(`Assert fail. ${msg}`);
  }
}

export function typedArrayToArrayBuffer(ta: TypedArray): ArrayBuffer {
  const ab = ta.buffer.slice(ta.byteOffset, ta.byteOffset + ta.byteLength);
  return ab as ArrayBuffer;
}

export function arrayToStr(ui8: Uint8Array): string {
  return String.fromCharCode(...ui8);
}

// A `Resolvable` is a Promise with the `reject` and `resolve` functions
// placed as methods on the promise object itself. It allows you to do:
//
//   const p = createResolvable<number>();
//   ...
//   p.resolve(42);
//
// It'd be prettier to make Resolvable a class that inherits from Promise,
// rather than an interface. This is possible in ES2016, however typescript
// produces broken code when targeting ES5 code.
// See https://github.com/Microsoft/TypeScript/issues/15202
// At the time of writing, the github issue is closed but the problem remains.
export interface Resolvable<T> extends Promise<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // tslint:disable-next-line:no-any
  reject: (reason?: any) => void;
}
export function createResolvable<T>(): Resolvable<T> {
  let methods;
  const promise = new Promise<T>((resolve, reject) => {
    methods = { resolve, reject };
  });
  return Object.assign(promise, methods) as Resolvable<T>;
}
