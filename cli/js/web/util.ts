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
