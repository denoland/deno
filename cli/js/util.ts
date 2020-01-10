// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { TypedArray } from "./types.ts";
import { window } from "./window.ts";

let logDebug = false;
let logSource = "JS";

// @internal
export function setLogDebug(debug: boolean, source?: string): void {
  logDebug = debug;
  if (source) {
    logSource = source;
  }
}

/** Debug logging for deno.
 * Enable with the `--log-debug` or `-D` command line flag.
 * @internal
 */
export function log(...args: unknown[]): void {
  if (logDebug) {
    // if we destructure `console` off `window` too early, we don't bind to
    // the right console, therefore we don't log anything out.
    window.console.log(`DEBUG ${logSource} -`, ...args);
  }
}

// @internal
export function assert(cond: unknown, msg = "assert"): asserts cond {
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
  throw new Error("Not implemented");
}

// @internal
export function unreachable(): never {
  throw new Error("Code not reachable");
}

// @internal
export function hexdump(u8: Uint8Array): string {
  return Array.prototype.map
    .call(u8, (x: number): string => {
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

const TypedArrayConstructor = Object.getPrototypeOf(Uint8Array);
export function isTypedArray(x: unknown): x is TypedArray {
  return x instanceof TypedArrayConstructor;
}

// Returns whether o is an object, not null, and not a function.
// @internal
export function isObject(o: unknown): o is object {
  return o != null && typeof o === "object";
}

// Returns whether o is iterable.
// @internal
export function isIterable<T, P extends keyof T, K extends T[P]>(
  o: T
): o is T & Iterable<[P, K]> {
  // checks for null and undefined
  if (o == null) {
    return false;
  }
  return (
    typeof ((o as unknown) as Iterable<[P, K]>)[Symbol.iterator] === "function"
  );
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
    writable: false
  });
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

/**
 * Determines whether an object has a property with the specified name.
 * Avoid calling prototype builtin `hasOwnProperty` for two reasons:
 *
 * 1. `hasOwnProperty` is defined on the object as something else:
 *
 *      const options = {
 *        ending: 'utf8',
 *        hasOwnProperty: 'foo'
 *      };
 *      options.hasOwnProperty('ending') // throws a TypeError
 *
 * 2. The object doesn't inherit from `Object.prototype`:
 *
 *       const options = Object.create(null);
 *       options.ending = 'utf8';
 *       options.hasOwnProperty('ending'); // throws a TypeError
 *
 * @param obj A Object.
 * @param v A property name.
 * @see https://eslint.org/docs/rules/no-prototype-builtins
 * @internal
 */
export function hasOwnProperty<T>(obj: T, v: PropertyKey): boolean {
  if (obj == null) {
    return false;
  }
  return Object.prototype.hasOwnProperty.call(obj, v);
}

/**
 * Split a number into two parts: lower 32 bit and higher 32 bit
 * (as if the number is represented as uint64.)
 *
 * @param n Number to split.
 * @internal
 */
export function splitNumberToParts(n: number): number[] {
  // JS bitwise operators (OR, SHIFT) operate as if number is uint32.
  const lower = n | 0;
  // This is also faster than Math.floor(n / 0x100000000) in V8.
  const higher = (n - lower) / 0x100000000;
  return [lower, higher];
}

// Constants used by `normalizeString` and `resolvePath`
export const CHAR_DOT = 46; /* . */
export const CHAR_FORWARD_SLASH = 47; /* / */

/** Resolves `.` and `..` elements in a path with directory names */
export function normalizeString(
  path: string,
  allowAboveRoot: boolean,
  separator: string,
  isPathSeparator: (code: number) => boolean
): string {
  let res = "";
  let lastSegmentLength = 0;
  let lastSlash = -1;
  let dots = 0;
  let code: number;
  for (let i = 0, len = path.length; i <= len; ++i) {
    if (i < len) code = path.charCodeAt(i);
    else if (isPathSeparator(code!)) break;
    else code = CHAR_FORWARD_SLASH;

    if (isPathSeparator(code)) {
      if (lastSlash === i - 1 || dots === 1) {
        // NOOP
      } else if (lastSlash !== i - 1 && dots === 2) {
        if (
          res.length < 2 ||
          lastSegmentLength !== 2 ||
          res.charCodeAt(res.length - 1) !== CHAR_DOT ||
          res.charCodeAt(res.length - 2) !== CHAR_DOT
        ) {
          if (res.length > 2) {
            const lastSlashIndex = res.lastIndexOf(separator);
            if (lastSlashIndex === -1) {
              res = "";
              lastSegmentLength = 0;
            } else {
              res = res.slice(0, lastSlashIndex);
              lastSegmentLength = res.length - 1 - res.lastIndexOf(separator);
            }
            lastSlash = i;
            dots = 0;
            continue;
          } else if (res.length === 2 || res.length === 1) {
            res = "";
            lastSegmentLength = 0;
            lastSlash = i;
            dots = 0;
            continue;
          }
        }
        if (allowAboveRoot) {
          if (res.length > 0) res += `${separator}..`;
          else res = "..";
          lastSegmentLength = 2;
        }
      } else {
        if (res.length > 0) res += separator + path.slice(lastSlash + 1, i);
        else res = path.slice(lastSlash + 1, i);
        lastSegmentLength = i - lastSlash - 1;
      }
      lastSlash = i;
      dots = 0;
    } else if (code === CHAR_DOT && dots !== -1) {
      ++dots;
    } else {
      dots = -1;
    }
  }
  return res;
}

/** Return the common path shared by the `paths`.
 *
 * @param paths The set of paths to compare.
 * @param sep An optional separator to use. Defaults to `/`.
 * @internal
 */
export function commonPath(paths: string[], sep = "/"): string {
  const [first = "", ...remaining] = paths;
  if (first === "" || remaining.length === 0) {
    return "";
  }
  const parts = first.split(sep);

  let endOfPrefix = parts.length;
  for (const path of remaining) {
    const compare = path.split(sep);
    for (let i = 0; i < endOfPrefix; i++) {
      if (compare[i] !== parts[i]) {
        endOfPrefix = i;
      }
    }

    if (endOfPrefix === 0) {
      return "";
    }
  }
  const prefix = parts.slice(0, endOfPrefix).join(sep);
  return prefix.endsWith(sep) ? prefix : `${prefix}${sep}`;
}

/** Utility function to turn the number of bytes into a human readable
 * unit */
export function humanFileSize(bytes: number): string {
  const thresh = 1000;
  if (Math.abs(bytes) < thresh) {
    return bytes + " B";
  }
  const units = ["kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
  let u = -1;
  do {
    bytes /= thresh;
    ++u;
  } while (Math.abs(bytes) >= thresh && u < units.length - 1);
  return `${bytes.toFixed(1)} ${units[u]}`;
}

// @internal
export function base64ToUint8Array(data: string): Uint8Array {
  const binString = window.atob(data);
  const size = binString.length;
  const bytes = new Uint8Array(size);
  for (let i = 0; i < size; i++) {
    bytes[i] = binString.charCodeAt(i);
  }
  return bytes;
}
