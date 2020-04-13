// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

let logDebug = false;
let logSource = "JS";

// @internal
export function setLogDebug(debug: boolean, source?: string): void {
  logDebug = debug;
  if (source) {
    logSource = source;
  }
}

export function log(...args: unknown[]): void {
  if (logDebug) {
    // if we destructure `console` off `globalThis` too early, we don't bind to
    // the right console, therefore we don't log anything out.
    globalThis.console.log(`DEBUG ${logSource} -`, ...args);
  }
}

// @internal
export function assert(cond: unknown, msg = "assert"): asserts cond {
  if (!cond) {
    throw Error(msg);
  }
}

export type ResolveFunction<T> = (value?: T | PromiseLike<T>) => void;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type RejectFunction = (reason?: any) => void;

export interface ResolvableMethods<T> {
  resolve: ResolveFunction<T>;
  reject: RejectFunction;
}

// @internal
export type Resolvable<T> = Promise<T> & ResolvableMethods<T>;

// @internal
export function createResolvable<T>(): Resolvable<T> {
  let resolve: ResolveFunction<T>;
  let reject: RejectFunction;
  const promise = new Promise<T>((res, rej): void => {
    resolve = res;
    reject = rej;
  }) as Resolvable<T>;
  promise.resolve = resolve!;
  promise.reject = reject!;
  return promise;
}

// @internal
export function notImplemented(): never {
  throw new Error("not implemented");
}

// @internal
export function immutableDefine(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  o: any,
  p: string | number | symbol,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  value: any
): void {
  Object.defineProperty(o, p, {
    value,
    configurable: false,
    writable: false,
  });
}
