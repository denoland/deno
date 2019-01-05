// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { TypedArray } from "./types";

let logDebug = false;

// @internal
export function setLogDebug(debug: boolean): void {
  logDebug = debug;
}

/** Debug logging for deno.
 * Enable with the `--log-debug` or `-D` command line flag.
 * @internal
 */
// tslint:disable-next-line:no-any
export function log(...args: any[]): void {
  if (logDebug) {
    console.log("DEBUG JS -", ...args);
  }
}

// @internal
export function assert(cond: boolean, msg = "assert") {
  if (!cond) {
    throw Error(msg);
  }
}

// @internal
export function typedArrayToArrayBuffer(ta: TypedArray): ArrayBuffer {
  const ab = ta.buffer.slice(ta.byteOffset, ta.byteOffset + ta.byteLength);
  return ab as ArrayBuffer;
}

// @internal
export function arrayToStr(ui8: Uint8Array): string {
  return String.fromCharCode(...ui8);
}

/** A `Resolvable` is a Promise with the `reject` and `resolve` functions
 * placed as methods on the promise object itself. It allows you to do:
 *
 *       const p = createResolvable<number>();
 *       // ...
 *       p.resolve(42);
 *
 * It'd be prettier to make `Resolvable` a class that inherits from `Promise`,
 * rather than an interface. This is possible in ES2016, however typescript
 * produces broken code when targeting ES5 code.
 *
 * At the time of writing, the GitHub issue is closed in favour of a proposed
 * solution that is awaiting feedback.
 *
 * @see https://github.com/Microsoft/TypeScript/issues/15202
 * @see https://github.com/Microsoft/TypeScript/issues/15397
 * @internal
 */

export interface ResolvableMethods<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // tslint:disable-next-line:no-any
  reject: (reason?: any) => void;
}

// @internal
export type Resolvable<T> = Promise<T> & ResolvableMethods<T>;

// @internal
export function createResolvable<T>(): Resolvable<T> {
  let methods: ResolvableMethods<T>;
  const promise = new Promise<T>((resolve, reject) => {
    methods = { resolve, reject };
  });
  // TypeScript doesn't know that the Promise callback occurs synchronously
  // therefore use of not null assertion (`!`)
  return Object.assign(promise, methods!) as Resolvable<T>;
}

// @internal
export function notImplemented(): never {
  throw new Error("Not implemented");
}

// @internal
export function unreachable(): never {
  throw new Error("Code not reachable");
}

// @internal
export function hexdump(u8: Uint8Array): string {
  return Array.prototype.map
    .call(u8, (x: number) => {
      return ("00" + x.toString(16)).slice(-2);
    })
    .join(" ");
}

// @internal
export function containsOnlyASCII(str: string): boolean {
  if (typeof str !== "string") {
    return false;
  }
  return /^[\x00-\x7F]*$/.test(str);
}

export interface Deferred {
  promise: Promise<void>;
  resolve: Function;
  reject: Function;
}

/** Create a wrapper around a promise that could be resolved externally.
 * TODO Do not expose this from "deno" namespace.
 */
export function deferred(): Deferred {
  let resolve: Function | undefined;
  let reject: Function | undefined;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return {
    promise,
    resolve: resolve!,
    reject: reject!
  };
}

// tslint:disable-next-line:variable-name
const TypedArrayConstructor = Object.getPrototypeOf(Uint8Array);
export function isTypedArray(x: unknown): x is TypedArray {
  return x instanceof TypedArrayConstructor;
}

// Returns whether o is an object, not null, and not a function.
// @internal
export function isObject(o: unknown): o is object {
  return o != null && typeof o === "object";
}

// @internal
export function requiredArguments(
  name: string,
  length: number,
  required: number
): void {
  if (length < required) {
    const errMsg = `${name} requires at least ${required} argument${
      required === 1 ? "" : "s"
    }, but only ${length} present`;
    throw new TypeError(errMsg);
  }
}

// Returns values from a WeakMap to emulate private properties in JavaScript
export function getPrivateValue<
  K extends object,
  V extends object,
  W extends keyof V
>(instance: K, weakMap: WeakMap<K, V>, key: W): V[W] {
  if (weakMap.has(instance)) {
    return weakMap.get(instance)![key];
  }
  throw new TypeError("Illegal invocation");
}
