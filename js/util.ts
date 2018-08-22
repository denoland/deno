// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { TypedArray } from "./types";

let logDebug = false;

export function setLogDebug(debug: boolean): void {
  logDebug = debug;
}

// Debug logging for deno. Enable with the --DEBUG command line flag.
// tslint:disable-next-line:no-any
export function log(...args: any[]): void {
  if (logDebug) {
    console.log("DEBUG JS -", ...args);
  }
}

export function assert(cond: boolean, msg = "assert") {
  if (!cond) {
    throw Error(msg);
  }
}

let cmdIdCounter = 0;
export function assignCmdId(): number {
  // TODO(piscisaureus) Safely re-use so they don't overflow.
  const cmdId = ++cmdIdCounter;
  assert(cmdId < 2 ** 32, "cmdId overflow");
  return cmdId;
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
export interface ResolvableMethods<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // tslint:disable-next-line:no-any
  reject: (reason?: any) => void;
}

export type Resolvable<T> = Promise<T> & ResolvableMethods<T>;

export function createResolvable<T>(): Resolvable<T> {
  let methods: ResolvableMethods<T>;
  const promise = new Promise<T>((resolve, reject) => {
    methods = { resolve, reject };
  });
  // TypeScript doesn't know that the Promise callback occurs synchronously
  // therefore use of not null assertion (`!`)
  return Object.assign(promise, methods!) as Resolvable<T>;
}

export function notImplemented(): never {
  throw new Error("Not implemented");
}

export function unreachable(): never {
  throw new Error("Code not reachable");
}
