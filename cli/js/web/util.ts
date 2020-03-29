// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export type TypedArray = Uint8Array | Float32Array | Int32Array;
const TypedArrayConstructor = Object.getPrototypeOf(Uint8Array);
export function isTypedArray(x: unknown): x is TypedArray {
  return x instanceof TypedArrayConstructor;
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
    writable: false,
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

export function hasOwnProperty<T>(obj: T, v: PropertyKey): boolean {
  if (obj == null) {
    return false;
  }
  return Object.prototype.hasOwnProperty.call(obj, v);
}

/** Returns whether o is iterable.
 *
 * @internal */
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
