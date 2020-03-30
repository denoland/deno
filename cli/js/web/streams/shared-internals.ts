// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

/* eslint-disable @typescript-eslint/no-explicit-any */
// TODO don't disable this warning

import { AbortSignal, QueuingStrategySizeCallback } from "../dom_types.ts";

// common stream fields

export const state_ = Symbol("state_");
export const storedError_ = Symbol("storedError_");

// ---------

export type ErrorResult = any;

// ---------

export function isInteger(value: number): boolean {
  if (!isFinite(value)) {
    // covers NaN, +Infinity and -Infinity
    return false;
  }
  const absValue = Math.abs(value);
  return Math.floor(absValue) === absValue;
}

export function isFiniteNonNegativeNumber(value: unknown): boolean {
  if (!(typeof value === "number" && isFinite(value))) {
    // covers NaN, +Infinity and -Infinity
    return false;
  }
  return value >= 0;
}

export function isAbortSignal(signal: any): signal is AbortSignal {
  if (typeof signal !== "object" || signal === null) {
    return false;
  }
  try {
    // TODO
    // calling signal.aborted() probably isn't the right way to perform this test
    // https://github.com/stardazed/sd-streams/blob/master/packages/streams/src/shared-internals.ts#L41
    signal.aborted();
    return true;
  } catch (err) {
    return false;
  }
}

export function invokeOrNoop<O extends object, P extends keyof O>(
  o: O,
  p: P,
  args: any[]
): any {
  // Assert: O is not undefined.
  // Assert: IsPropertyKey(P) is true.
  // Assert: args is a List.
  const method: Function | undefined = (o as any)[p]; // tslint:disable-line:ban-types
  if (method === undefined) {
    return undefined;
  }
  return Function.prototype.apply.call(method, o, args);
}

export function cloneArrayBuffer(
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

export function transferArrayBuffer(buffer: ArrayBufferLike): ArrayBuffer {
  // This would in a JS engine context detach the buffer's backing store and return
  // a new ArrayBuffer with the same backing store, invalidating `buffer`,
  // i.e. a move operation in C++ parlance.
  // Sadly ArrayBuffer.transfer is yet to be implemented by a single browser vendor.
  return buffer.slice(0); // copies instead of moves
}

export function copyDataBlockBytes(
  toBlock: ArrayBufferLike,
  toIndex: number,
  fromBlock: ArrayBufferLike,
  fromIndex: number,
  count: number
): void {
  new Uint8Array(toBlock, toIndex, count).set(
    new Uint8Array(fromBlock, fromIndex, count)
  );
}

// helper memoisation map for object values
// weak so it doesn't keep memoized versions of old objects indefinitely.
const objectCloneMemo = new WeakMap<object, object>();

let sharedArrayBufferSupported_: boolean | undefined;
function supportsSharedArrayBuffer(): boolean {
  if (sharedArrayBufferSupported_ === undefined) {
    try {
      new SharedArrayBuffer(16);
      sharedArrayBufferSupported_ = true;
    } catch (e) {
      sharedArrayBufferSupported_ = false;
    }
  }
  return sharedArrayBufferSupported_;
}

export function cloneValue(value: any): any {
  const valueType = typeof value;
  switch (valueType) {
    case "number":
    case "string":
    case "boolean":
    case "undefined":
    // @ts-ignore
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
      if (supportsSharedArrayBuffer() && value instanceof SharedArrayBuffer) {
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
        // Use DataViewConstructor type purely for type-checking, can be a DataView or TypedArray.
        // They use the same constructor signature, only DataView has a length in bytes and TypedArrays
        // use a length in terms of elements, so we adjust for that.
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

      // generic object
      const clonedObj = {} as any;
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
      // TODO this should be a DOMException,
      // https://github.com/stardazed/sd-streams/blob/master/packages/streams/src/shared-internals.ts#L171
      throw new Error("Uncloneable value in stream");
  }
}

export function promiseCall<F extends Function>(
  f: F,
  v: object | undefined,
  args: any[]
): Promise<any> {
  // tslint:disable-line:ban-types
  try {
    const result = Function.prototype.apply.call(f, v, args);
    return Promise.resolve(result);
  } catch (err) {
    return Promise.reject(err);
  }
}

export function createAlgorithmFromUnderlyingMethod<
  O extends object,
  K extends keyof O
>(obj: O, methodName: K, extraArgs: any[]): any {
  const method = obj[methodName];
  if (method === undefined) {
    return (): any => Promise.resolve(undefined);
  }
  if (typeof method !== "function") {
    throw new TypeError(`Field "${methodName}" is not a function.`);
  }
  return function (...fnArgs: any[]): any {
    return promiseCall(method, obj, fnArgs.concat(extraArgs));
  };
}

/*
Deprecated for now, all usages replaced by readableStreamCreateReadResult

function createIterResultObject<T>(value: T, done: boolean): IteratorResult<T> {
	return { value, done };
}
*/

export function validateAndNormalizeHighWaterMark(hwm: unknown): number {
  const highWaterMark = Number(hwm);
  if (isNaN(highWaterMark) || highWaterMark < 0) {
    throw new RangeError(
      "highWaterMark must be a valid, non-negative integer."
    );
  }
  return highWaterMark;
}

export function makeSizeAlgorithmFromSizeFunction<T>(
  sizeFn: undefined | ((chunk: T) => number)
): QueuingStrategySizeCallback<T> {
  if (typeof sizeFn !== "function" && typeof sizeFn !== "undefined") {
    throw new TypeError("size function must be undefined or a function");
  }
  return function (chunk: T): number {
    if (typeof sizeFn === "function") {
      return sizeFn(chunk);
    }
    return 1;
  };
}

// ----

export const enum ControlledPromiseState {
  Pending,
  Resolved,
  Rejected,
}

export interface ControlledPromise<V> {
  resolve(value?: V): void;
  reject(error: ErrorResult): void;
  promise: Promise<V>;
  state: ControlledPromiseState;
}

export function createControlledPromise<V>(): ControlledPromise<V> {
  const conProm = {
    state: ControlledPromiseState.Pending,
  } as ControlledPromise<V>;
  conProm.promise = new Promise<V>(function (resolve, reject) {
    conProm.resolve = function (v?: V): void {
      conProm.state = ControlledPromiseState.Resolved;
      resolve(v);
    };
    conProm.reject = function (e?: ErrorResult): void {
      conProm.state = ControlledPromiseState.Rejected;
      reject(e);
    };
  });
  return conProm;
}
