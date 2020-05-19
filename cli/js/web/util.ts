// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { DOMExceptionImpl as DOMException } from "./dom_exception.ts";

export type TypedArray =
  | Int8Array
  | Uint8Array
  | Uint8ClampedArray
  | Int16Array
  | Uint16Array
  | Int32Array
  | Uint32Array
  | Float32Array
  | Float64Array;

// @internal
export function isTypedArray(x: unknown): x is TypedArray {
  return ArrayBuffer.isView(x) && !(x instanceof DataView);
}

// @internal
export function isInvalidDate(x: Date): boolean {
  return isNaN(x.getTime());
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

// @internal
export function hasOwnProperty(obj: unknown, v: PropertyKey): boolean {
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

const objectCloneMemo = new WeakMap();

function cloneArrayBuffer(
  srcBuffer: ArrayBufferLike,
  srcByteOffset: number,
  srcLength: number,
  cloneConstructor: ArrayBufferConstructor | SharedArrayBufferConstructor
): InstanceType<typeof cloneConstructor> {
  // this function fudges the return type but SharedArrayBuffer is disabled for a while anyway
  return srcBuffer.slice(
    srcByteOffset,
    srcByteOffset + srcLength
  ) as InstanceType<typeof cloneConstructor>;
}

/** Clone a value in a similar way to structured cloning.  It is similar to a
 * StructureDeserialize(StructuredSerialize(...)). */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function cloneValue(value: any): any {
  switch (typeof value) {
    case "number":
    case "string":
    case "boolean":
    case "undefined":
    case "bigint":
      return value;
    case "object": {
      if (objectCloneMemo.has(value)) {
        return objectCloneMemo.get(value);
      }
      if (value === null) {
        return value;
      }
      if (value instanceof Date) {
        return new Date(value.valueOf());
      }
      if (value instanceof RegExp) {
        return new RegExp(value);
      }
      if (value instanceof SharedArrayBuffer) {
        return value;
      }
      if (value instanceof ArrayBuffer) {
        const cloned = cloneArrayBuffer(
          value,
          0,
          value.byteLength,
          ArrayBuffer
        );
        objectCloneMemo.set(value, cloned);
        return cloned;
      }
      if (ArrayBuffer.isView(value)) {
        const clonedBuffer = cloneValue(value.buffer) as ArrayBufferLike;
        // Use DataViewConstructor type purely for type-checking, can be a
        // DataView or TypedArray.  They use the same constructor signature,
        // only DataView has a length in bytes and TypedArrays use a length in
        // terms of elements, so we adjust for that.
        let length: number;
        if (value instanceof DataView) {
          length = value.byteLength;
        } else {
          length = (value as Uint8Array).length;
        }
        return new (value.constructor as DataViewConstructor)(
          clonedBuffer,
          value.byteOffset,
          length
        );
      }
      if (value instanceof Map) {
        const clonedMap = new Map();
        objectCloneMemo.set(value, clonedMap);
        value.forEach((v, k) => clonedMap.set(k, cloneValue(v)));
        return clonedMap;
      }
      if (value instanceof Set) {
        const clonedSet = new Map();
        objectCloneMemo.set(value, clonedSet);
        value.forEach((v, k) => clonedSet.set(k, cloneValue(v)));
        return clonedSet;
      }

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const clonedObj = {} as Record<string, any>;
      objectCloneMemo.set(value, clonedObj);
      const sourceKeys = Object.getOwnPropertyNames(value);
      for (const key of sourceKeys) {
        clonedObj[key] = cloneValue(value[key]);
      }
      return clonedObj;
    }
    case "symbol":
    case "function":
    default:
      throw new DOMException("Uncloneable value in stream", "DataCloneError");
  }
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
interface GenericConstructor<T = any> {
  prototype: T;
}

/** A helper function which ensures accessors are enumerable, as they normally
 * are not. */
export function defineEnumerableProps(
  Ctor: GenericConstructor,
  props: string[]
): void {
  for (const prop of props) {
    Reflect.defineProperty(Ctor.prototype, prop, { enumerable: true });
  }
}
