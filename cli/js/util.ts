// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { build } from "./build.ts";
import { exposeForTest } from "./internals.ts";

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
export class AssertionError extends Error {
  constructor(msg?: string) {
    super(msg);
    this.name = "AssertionError";
  }
}

// @internal
export function assert(cond: unknown, msg = "Assertion failed."): asserts cond {
  if (!cond) {
    throw new AssertionError(msg);
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

function pathFromURLWin32(url: URL): string {
  if (url.hostname !== "") {
    //TODO(actual-size) Node adds a punycode decoding step, we should consider adding this
    return `\\\\${url.hostname}${url.pathname}`;
  }

  const validPath = /^\/(?<driveLetter>[A-Za-z]):\//;
  const matches = validPath.exec(url.pathname);

  if (!matches?.groups?.driveLetter) {
    throw new TypeError("A URL with the file schema must be absolute.");
  }

  const pathname = url.pathname.replace(/\//g, "\\");
  // we don't want a leading slash on an absolute path in Windows
  return pathname.slice(1);
}

function pathFromURLPosix(url: URL): string {
  if (url.hostname !== "") {
    throw new TypeError(`Host must be empty.`);
  }

  return decodeURIComponent(url.pathname);
}

export function pathFromURL(pathOrUrl: string | URL): string {
  if (typeof pathOrUrl == "string") {
    try {
      pathOrUrl = new URL(pathOrUrl);
    } catch {}
  }
  if (pathOrUrl instanceof URL) {
    if (pathOrUrl.protocol != "file:") {
      throw new TypeError("Must be a path string or file URL.");
    }

    return build.os == "windows"
      ? pathFromURLWin32(pathOrUrl)
      : pathFromURLPosix(pathOrUrl);
  }
  return pathOrUrl;
}

exposeForTest("pathFromURL", pathFromURL);
