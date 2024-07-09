// deno-lint-ignore-file no-explicit-any camelcase ban-types
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Based on https://github.com/nodejs/node/blob/889ad35d3d41e376870f785b0c1b669cb732013d/typings/primordials.d.ts
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.
// This file subclasses and stores the JS builtins that come from the VM
// so that Node.js's builtin modules do not need to later look these up from
// the global proxy, which can be mutated by users.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/**
 * Primordials are a way to safely use globals without fear of global mutation
 * Generally, this means removing `this` parameter usage and instead using
 * a regular parameter:
 *
 * @example
 *
 * ```js
 * 'thing'.startsWith('hello');
 * ```
 *
 * becomes
 *
 * ```js
 * primordials.StringPrototypeStartsWith('thing', 'hello')
 * ```
 */
export namespace primordials {
  type UncurryThis<T extends (this: any, ...args: any[]) => any> = (
    self: ThisParameterType<T>,
    ...args: Parameters<T>
  ) => ReturnType<T>;
  type UncurryThisStaticApply<
    T extends (this: any, ...args: any[]) => any,
  > = (self: ThisParameterType<T>, args: Parameters<T>) => ReturnType<T>;
  type StaticApply<T extends (this: any, ...args: any[]) => any> = (
    args: Parameters<T>,
  ) => ReturnType<T>;

  export function uncurryThis<T extends (...args: any[]) => any>(
    fn: T,
  ): (self: ThisType<T>, ...args: Parameters<T>) => ReturnType<T>;
  export function applyBind<T extends (...args: any[]) => any>(
    fn: T,
  ): (self: ThisType<T>, args: Parameters<T>) => ReturnType<T>;

  // safe objects
  export function makeSafe<T extends NewableFunction>(
    unsafe: NewableFunction,
    safe: T,
  ): T;
  export const SafeMap: typeof globalThis.Map;
  export const SafeWeakMap: typeof globalThis.WeakMap;
  export const SafeSet: typeof globalThis.Set;
  export const SafeWeakSet: typeof globalThis.WeakSet;
  export const SafeFinalizationRegistry: typeof globalThis.FinalizationRegistry;
  export const SafeWeakRef: typeof globalThis.WeakRef;
  export const SafePromiseAll: typeof Promise.all;
  // NOTE: Uncomment the following functions when you need to use them
  // export const SafePromiseAllSettled: typeof Promise.allSettled;
  // export const SafePromiseAny: typeof Promise.any;
  // export const SafePromiseRace: typeof Promise.race;
  export const SafePromisePrototypeFinally: UncurryThis<
    Promise.prototype.finally
  >;
  export const SafeRegExp: typeof RegExp;

  // safe iterators
  export const SafeArrayIterator: new <T>(array: T[]) => IterableIterator<T>;
  export const SafeSetIterator: new <T>(set: Set<T>) => IterableIterator<T>;
  export const SafeMapIterator: new <K, V>(
    map: Map<K, V>,
  ) => IterableIterator<[K, V]>;
  export const SafeStringIterator: new (
    str: string,
  ) => IterableIterator<string>;

  // intrinsic objects
  export const indirectEval: typeof globalThis.eval;
  export const isNaN: typeof globalThis.isNaN;
  export const decodeURI: typeof globalThis.decodeURI;
  export const decodeURIComponent: typeof globalThis.decodeURIComponent;
  export const encodeURI: typeof globalThis.encodeURI;
  export const encodeURIComponent: typeof globalThis.encodeURIComponent;
  export const queueMicrotask: typeof globalThis.queueMicrotask;
  export const setQueueMicrotask: (
    queueMicrotask: typeof globalThis.queueMicrotask,
  ) => void;
  export const JSONParse: typeof JSON.parse;
  export const JSONStringify: typeof JSON.stringify;
  export const MathAbs: typeof Math.abs;
  export const MathAcos: typeof Math.acos;
  export const MathAcosh: typeof Math.acosh;
  export const MathAsin: typeof Math.asin;
  export const MathAsinh: typeof Math.asinh;
  export const MathAtan: typeof Math.atan;
  export const MathAtanh: typeof Math.atanh;
  export const MathAtan2: typeof Math.atan2;
  export const MathCeil: typeof Math.ceil;
  export const MathCbrt: typeof Math.cbrt;
  export const MathExpm1: typeof Math.expm1;
  export const MathClz32: typeof Math.clz32;
  export const MathCos: typeof Math.cos;
  export const MathCosh: typeof Math.cosh;
  export const MathExp: typeof Math.exp;
  export const MathFloor: typeof Math.floor;
  export const MathFround: typeof Math.fround;
  export const MathHypot: typeof Math.hypot;
  export const MathHypotApply: StaticApply<typeof Math.hypot>;
  export const MathImul: typeof Math.imul;
  export const MathLog: typeof Math.log;
  export const MathLog1p: typeof Math.log1p;
  export const MathLog2: typeof Math.log2;
  export const MathLog10: typeof Math.log10;
  export const MathMax: typeof Math.max;
  export const MathMaxApply: StaticApply<typeof Math.max>;
  export const MathMin: typeof Math.min;
  export const MathMinApply: StaticApply<typeof Math.min>;
  export const MathPow: typeof Math.pow;
  export const MathRandom: typeof Math.random;
  export const MathRound: typeof Math.round;
  export const MathSign: typeof Math.sign;
  export const MathSin: typeof Math.sin;
  export const MathSinh: typeof Math.sinh;
  export const MathSqrt: typeof Math.sqrt;
  export const MathTan: typeof Math.tan;
  export const MathTanh: typeof Math.tanh;
  export const MathTrunc: typeof Math.trunc;
  export const MathE: typeof Math.E;
  export const MathLN10: typeof Math.LN10;
  export const MathLN2: typeof Math.LN2;
  export const MathLOG10E: typeof Math.LOG10E;
  export const MathLOG2E: typeof Math.LOG2E;
  export const MathPI: typeof Math.PI;
  export const MathSQRT1_2: typeof Math.SQRT1_2;
  export const MathSQRT2: typeof Math.SQRT2;
  export const Proxy: typeof globalThis.Proxy;
  export const ProxyLength: typeof Proxy.length;
  export const ProxyName: typeof Proxy.name;
  export const ProxyRevocable: typeof Proxy.revocable;
  export const ReflectDefineProperty: typeof Reflect.defineProperty;
  export const ReflectDeleteProperty: typeof Reflect.deleteProperty;
  export const ReflectApply: typeof Reflect.apply;
  export const ReflectConstruct: typeof Reflect.construct;
  export const ReflectGet: typeof Reflect.get;
  export const ReflectGetOwnPropertyDescriptor:
    typeof Reflect.getOwnPropertyDescriptor;
  export const ReflectGetPrototypeOf: typeof Reflect.getPrototypeOf;
  export const ReflectHas: typeof Reflect.has;
  export const ReflectIsExtensible: typeof Reflect.isExtensible;
  export const ReflectOwnKeys: typeof Reflect.ownKeys;
  export const ReflectPreventExtensions: typeof Reflect.preventExtensions;
  export const ReflectSet: typeof Reflect.set;
  export const ReflectSetPrototypeOf: typeof Reflect.setPrototypeOf;
  export const AggregateError: typeof globalThis.AggregateError;
  export const AggregateErrorLength: typeof AggregateError.length;
  export const AggregateErrorName: typeof AggregateError.name;
  export const AggregateErrorPrototype: typeof AggregateError.prototype;
  export const Array: typeof globalThis.Array;
  export const ArrayLength: typeof Array.length;
  export const ArrayName: typeof Array.name;
  export const ArrayPrototype: typeof Array.prototype;
  export const ArrayIsArray: typeof Array.isArray;
  export const ArrayFrom: typeof Array.from;
  export const ArrayOf: typeof Array.of;
  export const ArrayOfApply: StaticApply<typeof Array.of>;
  export const ArrayPrototypeAt: UncurryThis<typeof Array.prototype.at>;
  export const ArrayPrototypeConcat: UncurryThis<
    typeof Array.prototype.concat
  >;
  export const ArrayPrototypeCopyWithin: UncurryThis<
    typeof Array.prototype.copyWithin
  >;
  export const ArrayPrototypeFill: UncurryThis<typeof Array.prototype.fill>;
  export const ArrayPrototypeFind: UncurryThis<typeof Array.prototype.find>;
  export const ArrayPrototypeFindIndex: UncurryThis<
    typeof Array.prototype.findIndex
  >;
  export const ArrayPrototypeLastIndexOf: UncurryThis<
    typeof Array.prototype.lastIndexOf
  >;
  export const ArrayPrototypePop: UncurryThis<typeof Array.prototype.pop>;
  export const ArrayPrototypePush: UncurryThis<typeof Array.prototype.push>;
  export const ArrayPrototypePushApply: UncurryThisStaticApply<
    typeof Array.prototype.push
  >;
  export const ArrayPrototypeReverse: UncurryThis<
    typeof Array.prototype.reverse
  >;
  export const ArrayPrototypeShift: UncurryThis<typeof Array.prototype.shift>;
  export const ArrayPrototypeUnshift: UncurryThis<
    typeof Array.prototype.unshift
  >;
  export const ArrayPrototypeUnshiftApply: UncurryThisStaticApply<
    typeof Array.prototype.unshift
  >;
  export const ArrayPrototypeSlice: UncurryThis<typeof Array.prototype.slice>;
  export const ArrayPrototypeSort: UncurryThis<typeof Array.prototype.sort>;
  export const ArrayPrototypeSplice: UncurryThis<
    typeof Array.prototype.splice
  >;
  export const ArrayPrototypeIncludes: UncurryThis<
    typeof Array.prototype.includes
  >;
  export const ArrayPrototypeIndexOf: UncurryThis<
    typeof Array.prototype.indexOf
  >;
  export const ArrayPrototypeJoin: UncurryThis<typeof Array.prototype.join>;
  export const ArrayPrototypeKeys: UncurryThis<typeof Array.prototype.keys>;
  export const ArrayPrototypeEntries: UncurryThis<
    typeof Array.prototype.entries
  >;
  export const ArrayPrototypeValues: UncurryThis<
    typeof Array.prototype.values
  >;
  export const ArrayPrototypeForEach: UncurryThis<
    typeof Array.prototype.forEach
  >;
  export const ArrayPrototypeFilter: UncurryThis<
    typeof Array.prototype.filter
  >;
  export const ArrayPrototypeFlat: UncurryThis<typeof Array.prototype.flat>;
  export const ArrayPrototypeFlatMap: UncurryThis<
    typeof Array.prototype.flatMap
  >;
  export const ArrayPrototypeMap: UncurryThis<typeof Array.prototype.map>;
  export const ArrayPrototypeEvery: UncurryThis<typeof Array.prototype.every>;
  export const ArrayPrototypeSome: UncurryThis<typeof Array.prototype.some>;
  export const ArrayPrototypeReduce: UncurryThis<
    typeof Array.prototype.reduce
  >;
  export const ArrayPrototypeReduceRight: UncurryThis<
    typeof Array.prototype.reduceRight
  >;
  export const ArrayPrototypeToLocaleString: UncurryThis<
    typeof Array.prototype.toLocaleString
  >;
  export const ArrayPrototypeToString: UncurryThis<
    typeof Array.prototype.toString
  >;
  export const ArrayBuffer: typeof globalThis.ArrayBuffer;
  export const ArrayBufferLength: typeof ArrayBuffer.length;
  export const ArrayBufferName: typeof ArrayBuffer.name;
  export const ArrayBufferIsView: typeof ArrayBuffer.isView;
  export const ArrayBufferPrototype: typeof ArrayBuffer.prototype;
  export const ArrayBufferPrototypeGetByteLength: (
    buffer: ArrayBuffer,
  ) => number;
  export const ArrayBufferPrototypeSlice: UncurryThis<
    typeof ArrayBuffer.prototype.slice
  >;
  export const BigInt: typeof globalThis.BigInt;
  export const BigIntLength: typeof BigInt.length;
  export const BigIntName: typeof BigInt.name;
  export const BigIntPrototype: typeof BigInt.prototype;
  export const BigIntAsUintN: typeof BigInt.asUintN;
  export const BigIntAsIntN: typeof BigInt.asIntN;
  export const BigIntPrototypeToLocaleString: UncurryThis<
    typeof BigInt.prototype.toLocaleString
  >;
  export const BigIntPrototypeToString: UncurryThis<
    typeof BigInt.prototype.toString
  >;
  export const BigIntPrototypeValueOf: UncurryThis<
    typeof BigInt.prototype.valueOf
  >;
  export const BigInt64Array: typeof globalThis.BigInt64Array;
  export const BigInt64ArrayLength: typeof BigInt64Array.length;
  export const BigInt64ArrayName: typeof BigInt64Array.name;
  export const BigInt64ArrayPrototype: typeof BigInt64Array.prototype;
  export const BigInt64ArrayBYTES_PER_ELEMENT:
    typeof BigInt64Array.BYTES_PER_ELEMENT;
  export const BigUint64Array: typeof globalThis.BigUint64Array;
  export const BigUint64ArrayLength: typeof BigUint64Array.length;
  export const BigUint64ArrayName: typeof BigUint64Array.name;
  export const BigUint64ArrayPrototype: typeof BigUint64Array.prototype;
  export const BigUint64ArrayBYTES_PER_ELEMENT:
    typeof BigUint64Array.BYTES_PER_ELEMENT;
  export const Boolean: typeof globalThis.Boolean;
  export const BooleanLength: typeof Boolean.length;
  export const BooleanName: typeof Boolean.name;
  export const BooleanPrototype: typeof Boolean.prototype;
  export const BooleanPrototypeToString: UncurryThis<
    typeof Boolean.prototype.toString
  >;
  export const BooleanPrototypeValueOf: UncurryThis<
    typeof Boolean.prototype.valueOf
  >;
  export const DataView: typeof globalThis.DataView;
  export const DataViewLength: typeof DataView.length;
  export const DataViewName: typeof DataView.name;
  export const DataViewPrototype: typeof DataView.prototype;
  export const DataViewPrototypeGetBuffer: (
    view: DataView,
  ) => ArrayBuffer | SharedArrayBuffer;
  export const DataViewPrototypeGetByteLength: (view: DataView) => number;
  export const DataViewPrototypeGetByteOffset: (view: DataView) => number;
  export const DataViewPrototypeGetInt8: UncurryThis<
    typeof DataView.prototype.getInt8
  >;
  export const DataViewPrototypeSetInt8: UncurryThis<
    typeof DataView.prototype.setInt8
  >;
  export const DataViewPrototypeGetUint8: UncurryThis<
    typeof DataView.prototype.getUint8
  >;
  export const DataViewPrototypeSetUint8: UncurryThis<
    typeof DataView.prototype.setUint8
  >;
  export const DataViewPrototypeGetInt16: UncurryThis<
    typeof DataView.prototype.getInt16
  >;
  export const DataViewPrototypeSetInt16: UncurryThis<
    typeof DataView.prototype.setInt16
  >;
  export const DataViewPrototypeGetUint16: UncurryThis<
    typeof DataView.prototype.getUint16
  >;
  export const DataViewPrototypeSetUint16: UncurryThis<
    typeof DataView.prototype.setUint16
  >;
  export const DataViewPrototypeGetInt32: UncurryThis<
    typeof DataView.prototype.getInt32
  >;
  export const DataViewPrototypeSetInt32: UncurryThis<
    typeof DataView.prototype.setInt32
  >;
  export const DataViewPrototypeGetUint32: UncurryThis<
    typeof DataView.prototype.getUint32
  >;
  export const DataViewPrototypeSetUint32: UncurryThis<
    typeof DataView.prototype.setUint32
  >;
  export const DataViewPrototypeGetFloat32: UncurryThis<
    typeof DataView.prototype.getFloat32
  >;
  export const DataViewPrototypeSetFloat32: UncurryThis<
    typeof DataView.prototype.setFloat32
  >;
  export const DataViewPrototypeGetFloat64: UncurryThis<
    typeof DataView.prototype.getFloat64
  >;
  export const DataViewPrototypeSetFloat64: UncurryThis<
    typeof DataView.prototype.setFloat64
  >;
  export const DataViewPrototypeGetBigInt64: UncurryThis<
    typeof DataView.prototype.getBigInt64
  >;
  export const DataViewPrototypeSetBigInt64: UncurryThis<
    typeof DataView.prototype.setBigInt64
  >;
  export const DataViewPrototypeGetBigUint64: UncurryThis<
    typeof DataView.prototype.getBigUint64
  >;
  export const DataViewPrototypeSetBigUint64: UncurryThis<
    typeof DataView.prototype.setBigUint64
  >;
  export const Date: typeof globalThis.Date;
  export const DateLength: typeof Date.length;
  export const DateName: typeof Date.name;
  export const DatePrototype: typeof Date.prototype;
  export const DateNow: typeof Date.now;
  export const DateParse: typeof Date.parse;
  export const DateUTC: typeof Date.UTC;
  export const DatePrototypeToString: UncurryThis<
    typeof Date.prototype.toString
  >;
  export const DatePrototypeToDateString: UncurryThis<
    typeof Date.prototype.toDateString
  >;
  export const DatePrototypeToTimeString: UncurryThis<
    typeof Date.prototype.toTimeString
  >;
  export const DatePrototypeToISOString: UncurryThis<
    typeof Date.prototype.toISOString
  >;
  export const DatePrototypeToUTCString: UncurryThis<
    typeof Date.prototype.toUTCString
  >;
  export const DatePrototypeGetDate: UncurryThis<
    typeof Date.prototype.getDate
  >;
  export const DatePrototypeSetDate: UncurryThis<
    typeof Date.prototype.setDate
  >;
  export const DatePrototypeGetDay: UncurryThis<typeof Date.prototype.getDay>;
  export const DatePrototypeGetFullYear: UncurryThis<
    typeof Date.prototype.getFullYear
  >;
  export const DatePrototypeSetFullYear: UncurryThis<
    typeof Date.prototype.setFullYear
  >;
  export const DatePrototypeGetHours: UncurryThis<
    typeof Date.prototype.getHours
  >;
  export const DatePrototypeSetHours: UncurryThis<
    typeof Date.prototype.setHours
  >;
  export const DatePrototypeGetMilliseconds: UncurryThis<
    typeof Date.prototype.getMilliseconds
  >;
  export const DatePrototypeSetMilliseconds: UncurryThis<
    typeof Date.prototype.setMilliseconds
  >;
  export const DatePrototypeGetMinutes: UncurryThis<
    typeof Date.prototype.getMinutes
  >;
  export const DatePrototypeSetMinutes: UncurryThis<
    typeof Date.prototype.setMinutes
  >;
  export const DatePrototypeGetMonth: UncurryThis<
    typeof Date.prototype.getMonth
  >;
  export const DatePrototypeSetMonth: UncurryThis<
    typeof Date.prototype.setMonth
  >;
  export const DatePrototypeGetSeconds: UncurryThis<
    typeof Date.prototype.getSeconds
  >;
  export const DatePrototypeSetSeconds: UncurryThis<
    typeof Date.prototype.setSeconds
  >;
  export const DatePrototypeGetTime: UncurryThis<
    typeof Date.prototype.getTime
  >;
  export const DatePrototypeSetTime: UncurryThis<
    typeof Date.prototype.setTime
  >;
  export const DatePrototypeGetTimezoneOffset: UncurryThis<
    typeof Date.prototype.getTimezoneOffset
  >;
  export const DatePrototypeGetUTCDate: UncurryThis<
    typeof Date.prototype.getUTCDate
  >;
  export const DatePrototypeSetUTCDate: UncurryThis<
    typeof Date.prototype.setUTCDate
  >;
  export const DatePrototypeGetUTCDay: UncurryThis<
    typeof Date.prototype.getUTCDay
  >;
  export const DatePrototypeGetUTCFullYear: UncurryThis<
    typeof Date.prototype.getUTCFullYear
  >;
  export const DatePrototypeSetUTCFullYear: UncurryThis<
    typeof Date.prototype.setUTCFullYear
  >;
  export const DatePrototypeGetUTCHours: UncurryThis<
    typeof Date.prototype.getUTCHours
  >;
  export const DatePrototypeSetUTCHours: UncurryThis<
    typeof Date.prototype.setUTCHours
  >;
  export const DatePrototypeGetUTCMilliseconds: UncurryThis<
    typeof Date.prototype.getUTCMilliseconds
  >;
  export const DatePrototypeSetUTCMilliseconds: UncurryThis<
    typeof Date.prototype.setUTCMilliseconds
  >;
  export const DatePrototypeGetUTCMinutes: UncurryThis<
    typeof Date.prototype.getUTCMinutes
  >;
  export const DatePrototypeSetUTCMinutes: UncurryThis<
    typeof Date.prototype.setUTCMinutes
  >;
  export const DatePrototypeGetUTCMonth: UncurryThis<
    typeof Date.prototype.getUTCMonth
  >;
  export const DatePrototypeSetUTCMonth: UncurryThis<
    typeof Date.prototype.setUTCMonth
  >;
  export const DatePrototypeGetUTCSeconds: UncurryThis<
    typeof Date.prototype.getUTCSeconds
  >;
  export const DatePrototypeSetUTCSeconds: UncurryThis<
    typeof Date.prototype.setUTCSeconds
  >;
  export const DatePrototypeValueOf: UncurryThis<
    typeof Date.prototype.valueOf
  >;
  export const DatePrototypeToJSON: UncurryThis<typeof Date.prototype.toJSON>;
  export const DatePrototypeToLocaleString: UncurryThis<
    typeof Date.prototype.toLocaleString
  >;
  export const DatePrototypeToLocaleDateString: UncurryThis<
    typeof Date.prototype.toLocaleDateString
  >;
  export const DatePrototypeToLocaleTimeString: UncurryThis<
    typeof Date.prototype.toLocaleTimeString
  >;
  export const Error: typeof globalThis.Error;
  export const ErrorLength: typeof Error.length;
  export const ErrorName: typeof Error.name;
  export const ErrorPrototype: typeof Error.prototype;
  export const ErrorCaptureStackTrace: typeof Error.captureStackTrace;
  export const ErrorStackTraceLimit: typeof Error.stackTraceLimit;
  export const ErrorPrototypeToString: UncurryThis<
    typeof Error.prototype.toString
  >;
  export const EvalError: typeof globalThis.EvalError;
  export const EvalErrorLength: typeof EvalError.length;
  export const EvalErrorName: typeof EvalError.name;
  export const EvalErrorPrototype: typeof EvalError.prototype;
  export const FinalizationRegistry: typeof globalThis.FinalizationRegistry;
  export const FinalizationRegistryLength: typeof FinalizationRegistry.length;
  export const FinalizationRegistryName: typeof FinalizationRegistry.name;
  export const FinalizationRegistryPrototype:
    typeof FinalizationRegistry.prototype;
  export const FinalizationRegistryPrototypeRegister: UncurryThis<
    typeof FinalizationRegistry.prototype.register
  >;
  export const FinalizationRegistryPrototypeUnregister: UncurryThis<
    typeof FinalizationRegistry.prototype.unregister
  >;
  export const Float32Array: typeof globalThis.Float32Array;
  export const Float32ArrayLength: typeof Float32Array.length;
  export const Float32ArrayName: typeof Float32Array.name;
  export const Float32ArrayPrototype: typeof Float32Array.prototype;
  export const Float32ArrayBYTES_PER_ELEMENT:
    typeof Float32Array.BYTES_PER_ELEMENT;
  export const Float64Array: typeof globalThis.Float64Array;
  export const Float64ArrayLength: typeof Float64Array.length;
  export const Float64ArrayName: typeof Float64Array.name;
  export const Float64ArrayPrototype: typeof Float64Array.prototype;
  export const Float64ArrayBYTES_PER_ELEMENT:
    typeof Float64Array.BYTES_PER_ELEMENT;
  export const Function: typeof globalThis.Function;
  export const FunctionLength: typeof Function.length;
  export const FunctionName: typeof Function.name;
  export const FunctionPrototype: typeof Function.prototype;
  export const FunctionPrototypeApply: UncurryThis<
    typeof Function.prototype.apply
  >;
  export const FunctionPrototypeBind: UncurryThis<
    typeof Function.prototype.bind
  >;
  export const FunctionPrototypeCall: UncurryThis<
    typeof Function.prototype.call
  >;
  export const FunctionPrototypeToString: UncurryThis<
    typeof Function.prototype.toString
  >;
  export const Int16Array: typeof globalThis.Int16Array;
  export const Int16ArrayLength: typeof Int16Array.length;
  export const Int16ArrayName: typeof Int16Array.name;
  export const Int16ArrayPrototype: typeof Int16Array.prototype;
  export const Int16ArrayBYTES_PER_ELEMENT: typeof Int16Array.BYTES_PER_ELEMENT;
  export const Int32Array: typeof globalThis.Int32Array;
  export const Int32ArrayLength: typeof Int32Array.length;
  export const Int32ArrayName: typeof Int32Array.name;
  export const Int32ArrayPrototype: typeof Int32Array.prototype;
  export const Int32ArrayBYTES_PER_ELEMENT: typeof Int32Array.BYTES_PER_ELEMENT;
  export const Int8Array: typeof globalThis.Int8Array;
  export const Int8ArrayLength: typeof Int8Array.length;
  export const Int8ArrayName: typeof Int8Array.name;
  export const Int8ArrayPrototype: typeof Int8Array.prototype;
  export const Int8ArrayBYTES_PER_ELEMENT: typeof Int8Array.BYTES_PER_ELEMENT;
  export const Map: typeof globalThis.Map;
  export const MapLength: typeof Map.length;
  export const MapName: typeof Map.name;
  export const MapPrototype: typeof Map.prototype;
  export const MapPrototypeGetSize: (map: Map<any, any>) => number;
  export const MapPrototypeGet: UncurryThis<typeof Map.prototype.get>;
  export const MapPrototypeSet: UncurryThis<typeof Map.prototype.set>;
  export const MapPrototypeHas: UncurryThis<typeof Map.prototype.has>;
  export const MapPrototypeDelete: UncurryThis<typeof Map.prototype.delete>;
  export const MapPrototypeClear: UncurryThis<typeof Map.prototype.clear>;
  export const MapPrototypeEntries: UncurryThis<typeof Map.prototype.entries>;
  export const MapPrototypeForEach: UncurryThis<typeof Map.prototype.forEach>;
  export const MapPrototypeKeys: UncurryThis<typeof Map.prototype.keys>;
  export const MapPrototypeValues: UncurryThis<typeof Map.prototype.values>;
  export const Number: typeof globalThis.Number;
  export const NumberLength: typeof Number.length;
  export const NumberName: typeof Number.name;
  export const NumberPrototype: typeof Number.prototype;
  export const NumberIsFinite: typeof Number.isFinite;
  export const NumberIsInteger: typeof Number.isInteger;
  export const NumberIsNaN: typeof Number.isNaN;
  export const NumberIsSafeInteger: typeof Number.isSafeInteger;
  export const NumberParseFloat: typeof Number.parseFloat;
  export const NumberParseInt: typeof Number.parseInt;
  export const NumberMAX_VALUE: typeof Number.MAX_VALUE;
  export const NumberMIN_VALUE: typeof Number.MIN_VALUE;
  export const NumberNaN: typeof Number.NaN;
  export const NumberNEGATIVE_INFINITY: typeof Number.NEGATIVE_INFINITY;
  export const NumberPOSITIVE_INFINITY: typeof Number.POSITIVE_INFINITY;
  export const NumberMAX_SAFE_INTEGER: typeof Number.MAX_SAFE_INTEGER;
  export const NumberMIN_SAFE_INTEGER: typeof Number.MIN_SAFE_INTEGER;
  export const NumberEPSILON: typeof Number.EPSILON;
  export const NumberPrototypeToExponential: UncurryThis<
    typeof Number.prototype.toExponential
  >;
  export const NumberPrototypeToFixed: UncurryThis<
    typeof Number.prototype.toFixed
  >;
  export const NumberPrototypeToPrecision: UncurryThis<
    typeof Number.prototype.toPrecision
  >;
  export const NumberPrototypeToString: UncurryThis<
    typeof Number.prototype.toString
  >;
  export const NumberPrototypeValueOf: UncurryThis<
    typeof Number.prototype.valueOf
  >;
  export const NumberPrototypeToLocaleString: UncurryThis<
    typeof Number.prototype.toLocaleString
  >;
  export const Object: typeof globalThis.Object;
  export const ObjectLength: typeof Object.length;
  export const ObjectName: typeof Object.name;
  export const ObjectAssign: typeof Object.assign;
  export const ObjectGetOwnPropertyDescriptor:
    typeof Object.getOwnPropertyDescriptor;
  export const ObjectGetOwnPropertyDescriptors:
    typeof Object.getOwnPropertyDescriptors;
  export const ObjectGetOwnPropertyNames: typeof Object.getOwnPropertyNames;
  export const ObjectGetOwnPropertySymbols: typeof Object.getOwnPropertySymbols;
  export const ObjectHasOwn: typeof Object.hasOwn;
  export const ObjectIs: typeof Object.is;
  export const ObjectPreventExtensions: typeof Object.preventExtensions;
  export const ObjectSeal: typeof Object.seal;
  export const ObjectCreate: typeof Object.create;
  export const ObjectDefineProperties: typeof Object.defineProperties;
  export const ObjectDefineProperty: typeof Object.defineProperty;
  export const ObjectFreeze: typeof Object.freeze;
  export const ObjectGetPrototypeOf: typeof Object.getPrototypeOf;
  export const ObjectSetPrototypeOf: typeof Object.setPrototypeOf;
  export const ObjectIsExtensible: typeof Object.isExtensible;
  export const ObjectIsFrozen: typeof Object.isFrozen;
  export const ObjectIsSealed: typeof Object.isSealed;
  export const ObjectKeys: typeof Object.keys;
  export const ObjectEntries: typeof Object.entries;
  export const ObjectFromEntries: typeof Object.fromEntries;
  export const ObjectValues: typeof Object.values;
  export const ObjectPrototype: typeof Object.prototype;
  export const ObjectPrototypeHasOwnProperty: UncurryThis<
    typeof Object.prototype.hasOwnProperty
  >;
  export const ObjectPrototypeIsPrototypeOf: UncurryThis<
    typeof Object.prototype.isPrototypeOf
  >;
  export const ObjectPrototypePropertyIsEnumerable: UncurryThis<
    typeof Object.prototype.propertyIsEnumerable
  >;
  export const ObjectPrototypeToString: UncurryThis<
    typeof Object.prototype.toString
  >;
  export const ObjectPrototypeValueOf: UncurryThis<
    typeof Object.prototype.valueOf
  >;
  export const ObjectPrototypeToLocaleString: UncurryThis<
    typeof Object.prototype.toLocaleString
  >;
  export const RangeError: typeof globalThis.RangeError;
  export const RangeErrorLength: typeof RangeError.length;
  export const RangeErrorName: typeof RangeError.name;
  export const RangeErrorPrototype: typeof RangeError.prototype;
  export const ReferenceError: typeof globalThis.ReferenceError;
  export const ReferenceErrorLength: typeof ReferenceError.length;
  export const ReferenceErrorName: typeof ReferenceError.name;
  export const ReferenceErrorPrototype: typeof ReferenceError.prototype;
  export const RegExp: typeof globalThis.RegExp;
  export const RegExpLength: typeof RegExp.length;
  export const RegExpName: typeof RegExp.name;
  export const RegExpPrototype: typeof RegExp.prototype;
  export const RegExpPrototypeExec: UncurryThis<typeof RegExp.prototype.exec>;
  export const RegExpPrototypeCompile: UncurryThis<
    typeof RegExp.prototype.compile
  >;
  export const RegExpPrototypeToString: UncurryThis<
    typeof RegExp.prototype.toString
  >;
  export const RegExpPrototypeTest: UncurryThis<typeof RegExp.prototype.test>;
  export const Set: typeof globalThis.Set;
  export const SetLength: typeof Set.length;
  export const SetName: typeof Set.name;
  export const SetPrototype: typeof Set.prototype;
  export const SetPrototypeGetSize: (set: Set<any>) => number;
  export const SetPrototypeHas: UncurryThis<typeof Set.prototype.has>;
  export const SetPrototypeAdd: UncurryThis<typeof Set.prototype.add>;
  export const SetPrototypeDelete: UncurryThis<typeof Set.prototype.delete>;
  export const SetPrototypeClear: UncurryThis<typeof Set.prototype.clear>;
  export const SetPrototypeEntries: UncurryThis<typeof Set.prototype.entries>;
  export const SetPrototypeForEach: UncurryThis<typeof Set.prototype.forEach>;
  export const SetPrototypeValues: UncurryThis<typeof Set.prototype.values>;
  export const SetPrototypeKeys: UncurryThis<typeof Set.prototype.keys>;
  export const String: typeof globalThis.String;
  export const StringLength: typeof String.length;
  export const StringName: typeof String.name;
  export const StringPrototype: typeof String.prototype;
  export const StringFromCharCode: typeof String.fromCharCode;
  export const StringFromCodePoint: typeof String.fromCodePoint;
  export const StringRaw: typeof String.raw;
  export const StringPrototypeAnchor: UncurryThis<
    typeof String.prototype.anchor
  >;
  export const StringPrototypeBig: UncurryThis<typeof String.prototype.big>;
  export const StringPrototypeBlink: UncurryThis<
    typeof String.prototype.blink
  >;
  export const StringPrototypeBold: UncurryThis<typeof String.prototype.bold>;
  export const StringPrototypeCharAt: UncurryThis<
    typeof String.prototype.charAt
  >;
  export const StringPrototypeCharCodeAt: UncurryThis<
    typeof String.prototype.charCodeAt
  >;
  export const StringPrototypeCodePointAt: UncurryThis<
    typeof String.prototype.codePointAt
  >;
  export const StringPrototypeConcat: UncurryThis<
    typeof String.prototype.concat
  >;
  export const StringPrototypeConcatApply: UncurryThisStaticApply<
    typeof String.prototype.concat
  >;
  export const StringPrototypeEndsWith: UncurryThis<
    typeof String.prototype.endsWith
  >;
  export const StringPrototypeFontcolor: UncurryThis<
    typeof String.prototype.fontcolor
  >;
  export const StringPrototypeFontsize: UncurryThis<
    typeof String.prototype.fontsize
  >;
  export const StringPrototypeFixed: UncurryThis<
    typeof String.prototype.fixed
  >;
  export const StringPrototypeIncludes: UncurryThis<
    typeof String.prototype.includes
  >;
  export const StringPrototypeIndexOf: UncurryThis<
    typeof String.prototype.indexOf
  >;
  export const StringPrototypeItalics: UncurryThis<
    typeof String.prototype.italics
  >;
  export const StringPrototypeLastIndexOf: UncurryThis<
    typeof String.prototype.lastIndexOf
  >;
  export const StringPrototypeLink: UncurryThis<typeof String.prototype.link>;
  export const StringPrototypeLocaleCompare: UncurryThis<
    typeof String.prototype.localeCompare
  >;
  export const StringPrototypeMatch: UncurryThis<
    typeof String.prototype.match
  >;
  export const StringPrototypeMatchAll: UncurryThis<
    typeof String.prototype.matchAll
  >;
  export const StringPrototypeNormalize: UncurryThis<
    typeof String.prototype.normalize
  >;
  export const StringPrototypePadEnd: UncurryThis<
    typeof String.prototype.padEnd
  >;
  export const StringPrototypePadStart: UncurryThis<
    typeof String.prototype.padStart
  >;
  export const StringPrototypeRepeat: UncurryThis<
    typeof String.prototype.repeat
  >;
  export const StringPrototypeReplace: UncurryThis<
    typeof String.prototype.replace
  >;
  export const StringPrototypeSearch: UncurryThis<
    typeof String.prototype.search
  >;
  export const StringPrototypeSlice: UncurryThis<
    typeof String.prototype.slice
  >;
  export const StringPrototypeSmall: UncurryThis<
    typeof String.prototype.small
  >;
  export const StringPrototypeSplit: UncurryThis<
    typeof String.prototype.split
  >;
  export const StringPrototypeStrike: UncurryThis<
    typeof String.prototype.strike
  >;
  export const StringPrototypeSub: UncurryThis<typeof String.prototype.sub>;
  export const StringPrototypeSubstr: UncurryThis<
    typeof String.prototype.substr
  >;
  export const StringPrototypeSubstring: UncurryThis<
    typeof String.prototype.substring
  >;
  export const StringPrototypeSup: UncurryThis<typeof String.prototype.sup>;
  export const StringPrototypeStartsWith: UncurryThis<
    typeof String.prototype.startsWith
  >;
  export const StringPrototypeToString: UncurryThis<
    typeof String.prototype.toString
  >;
  export const StringPrototypeTrim: UncurryThis<typeof String.prototype.trim>;
  export const StringPrototypeTrimStart: UncurryThis<
    typeof String.prototype.trimStart
  >;
  export const StringPrototypeTrimLeft: UncurryThis<
    typeof String.prototype.trimLeft
  >;
  export const StringPrototypeTrimEnd: UncurryThis<
    typeof String.prototype.trimEnd
  >;
  export const StringPrototypeTrimRight: UncurryThis<
    typeof String.prototype.trimRight
  >;
  export const StringPrototypeToLocaleLowerCase: UncurryThis<
    typeof String.prototype.toLocaleLowerCase
  >;
  export const StringPrototypeToLocaleUpperCase: UncurryThis<
    typeof String.prototype.toLocaleUpperCase
  >;
  export const StringPrototypeToLowerCase: UncurryThis<
    typeof String.prototype.toLowerCase
  >;
  export const StringPrototypeToUpperCase: UncurryThis<
    typeof String.prototype.toUpperCase
  >;
  export const StringPrototypeValueOf: UncurryThis<
    typeof String.prototype.valueOf
  >;
  export const StringPrototypeReplaceAll: UncurryThis<
    typeof String.prototype.replaceAll
  >;
  export const Symbol: typeof globalThis.Symbol;
  export const SymbolLength: typeof Symbol.length;
  export const SymbolName: typeof Symbol.name;
  export const SymbolPrototype: typeof Symbol.prototype;
  export const SymbolPrototypeGetDescription: (symbol: symbol) => string;
  export const SymbolFor: typeof Symbol.for;
  export const SymbolKeyFor: typeof Symbol.keyFor;
  export const SymbolAsyncIterator: typeof Symbol.asyncIterator;
  export const SymbolHasInstance: typeof Symbol.hasInstance;
  export const SymbolIsConcatSpreadable: typeof Symbol.isConcatSpreadable;
  export const SymbolIterator: typeof Symbol.iterator;
  export const SymbolMatch: typeof Symbol.match;
  export const SymbolMatchAll: typeof Symbol.matchAll;
  export const SymbolReplace: typeof Symbol.replace;
  export const SymbolSearch: typeof Symbol.search;
  export const SymbolSpecies: typeof Symbol.species;
  export const SymbolSplit: typeof Symbol.split;
  export const SymbolToPrimitive: typeof Symbol.toPrimitive;
  export const SymbolToStringTag: typeof Symbol.toStringTag;
  export const SymbolUnscopables: typeof Symbol.unscopables;
  export const SymbolPrototypeToString: UncurryThis<
    typeof Symbol.prototype.toString
  >;
  export const SymbolPrototypeValueOf: UncurryThis<
    typeof Symbol.prototype.valueOf
  >;
  export const SyntaxError: typeof globalThis.SyntaxError;
  export const SyntaxErrorLength: typeof SyntaxError.length;
  export const SyntaxErrorName: typeof SyntaxError.name;
  export const SyntaxErrorPrototype: typeof SyntaxError.prototype;
  export const TypeError: typeof globalThis.TypeError;
  export const TypeErrorLength: typeof TypeError.length;
  export const TypeErrorName: typeof TypeError.name;
  export const TypeErrorPrototype: typeof TypeError.prototype;
  export const URIError: typeof globalThis.URIError;
  export const URIErrorLength: typeof URIError.length;
  export const URIErrorName: typeof URIError.name;
  export const URIErrorPrototype: typeof URIError.prototype;
  export const Uint16Array: typeof globalThis.Uint16Array;
  export const Uint16ArrayLength: typeof Uint16Array.length;
  export const Uint16ArrayName: typeof Uint16Array.name;
  export const Uint16ArrayPrototype: typeof Uint16Array.prototype;
  export const Uint16ArrayBYTES_PER_ELEMENT:
    typeof Uint16Array.BYTES_PER_ELEMENT;
  export const Uint32Array: typeof globalThis.Uint32Array;
  export const Uint32ArrayLength: typeof Uint32Array.length;
  export const Uint32ArrayName: typeof Uint32Array.name;
  export const Uint32ArrayPrototype: typeof Uint32Array.prototype;
  export const Uint32ArrayBYTES_PER_ELEMENT:
    typeof Uint32Array.BYTES_PER_ELEMENT;
  export const Uint8Array: typeof globalThis.Uint8Array;
  export const Uint8ArrayLength: typeof Uint8Array.length;
  export const Uint8ArrayName: typeof Uint8Array.name;
  export const Uint8ArrayPrototype: typeof Uint8Array.prototype;
  export const Uint8ArrayBYTES_PER_ELEMENT: typeof Uint8Array.BYTES_PER_ELEMENT;
  export const Uint8ClampedArray: typeof globalThis.Uint8ClampedArray;
  export const Uint8ClampedArrayLength: typeof Uint8ClampedArray.length;
  export const Uint8ClampedArrayName: typeof Uint8ClampedArray.name;
  export const Uint8ClampedArrayPrototype: typeof Uint8ClampedArray.prototype;
  export const Uint8ClampedArrayBYTES_PER_ELEMENT:
    typeof Uint8ClampedArray.BYTES_PER_ELEMENT;
  export const WeakMap: typeof globalThis.WeakMap;
  export const WeakMapLength: typeof WeakMap.length;
  export const WeakMapName: typeof WeakMap.name;
  export const WeakMapPrototype: typeof WeakMap.prototype;
  export const WeakMapPrototypeDelete: UncurryThis<
    typeof WeakMap.prototype.delete
  >;
  export const WeakMapPrototypeGet: UncurryThis<typeof WeakMap.prototype.get>;
  export const WeakMapPrototypeSet: UncurryThis<typeof WeakMap.prototype.set>;
  export const WeakMapPrototypeHas: UncurryThis<typeof WeakMap.prototype.has>;
  export const WeakRef: typeof globalThis.WeakRef;
  export const WeakRefLength: typeof WeakRef.length;
  export const WeakRefName: typeof WeakRef.name;
  export const WeakRefPrototype: typeof WeakRef.prototype;
  export const WeakRefPrototypeDeref: UncurryThis<
    typeof WeakRef.prototype.deref
  >;
  export const WeakSet: typeof globalThis.WeakSet;
  export const WeakSetLength: typeof WeakSet.length;
  export const WeakSetName: typeof WeakSet.name;
  export const WeakSetPrototype: typeof WeakSet.prototype;
  export const WeakSetPrototypeDelete: UncurryThis<
    typeof WeakSet.prototype.delete
  >;
  export const WeakSetPrototypeHas: UncurryThis<typeof WeakSet.prototype.has>;
  export const WeakSetPrototypeAdd: UncurryThis<typeof WeakSet.prototype.add>;
  export const Promise: typeof globalThis.Promise;
  export const PromiseLength: typeof Promise.length;
  export const PromiseName: typeof Promise.name;
  export const PromisePrototype: typeof Promise.prototype;
  export const PromiseAll: typeof Promise.all;
  export const PromiseRace: typeof Promise.race;
  export const PromiseResolve: typeof Promise.resolve;
  export const PromiseReject: typeof Promise.reject;
  export const PromiseAllSettled: typeof Promise.allSettled;
  export const PromiseAny: typeof Promise.any;
  export const PromisePrototypeThen: UncurryThis<
    typeof Promise.prototype.then
  >;
  export const PromisePrototypeCatch: UncurryThis<
    typeof Promise.prototype.catch
  >;
  export const PromisePrototypeFinally: UncurryThis<
    typeof Promise.prototype.finally
  >;

  // abstract intrinsic objects
  export const ArrayIteratorPrototypeNext: <T>(
    iterator: IterableIterator<T>,
  ) => IteratorResult<T>;
  export const SetIteratorPrototypeNext: <T>(
    iterator: IterableIterator<T>,
  ) => IteratorResult<T>;
  export const MapIteratorPrototypeNext: <T>(
    iterator: IterableIterator<T>,
  ) => IteratorResult<T>;
  export const StringIteratorPrototypeNext: <T>(
    iterator: IterableIterator<T>,
  ) => IteratorResult<T>;
  export const GeneratorPrototypeNext: <T>(
    generator: Generator<T>,
  ) => IteratorResult<T>;
  export const AsyncGeneratorPrototypeNext: <T>(
    asyncGenerator: AsyncGenerator<T>,
  ) => Promise<IteratorResult<T>>;
  export const TypedArrayFrom: (
    constructor: Uint8ArrayConstructor,
    arrayLike: ArrayLike<number>,
  ) => Uint8Array;
  export const TypedArrayPrototypeGetBuffer: (
    array: Uint8Array,
  ) => ArrayBuffer | SharedArrayBuffer;
  export const TypedArrayPrototypeGetByteLength: (
    array: Uint8Array,
  ) => number;
  export const TypedArrayPrototypeGetByteOffset: (
    array: Uint8Array,
  ) => number;
  export const TypedArrayPrototypeGetLength: (array: Uint8Array) => number;
  export const TypedArrayPrototypeGetSymbolToStringTag: (
    v: unknown,
  ) => string | undefined;
  export const TypedArrayPrototypeCopyWithin: UncurryThis<
    typeof Uint8Array.prototype.copyWithin
  >;
  export const TypedArrayPrototypeEvery: UncurryThis<
    typeof Uint8Array.prototype.every
  >;
  export const TypedArrayPrototypeFill: UncurryThis<
    typeof Uint8Array.prototype.fill
  >;
  export const TypedArrayPrototypeFilter: UncurryThis<
    typeof Uint8Array.prototype.filter
  >;
  export const TypedArrayPrototypeFind: UncurryThis<
    typeof Uint8Array.prototype.find
  >;
  export const TypedArrayPrototypeFindIndex: UncurryThis<
    typeof Uint8Array.prototype.findIndex
  >;
  export const TypedArrayPrototypeForEach: UncurryThis<
    typeof Uint8Array.prototype.forEach
  >;
  export const TypedArrayPrototypeIndexOf: UncurryThis<
    typeof Uint8Array.prototype.indexOf
  >;
  export const TypedArrayPrototypeJoin: UncurryThis<
    typeof Uint8Array.prototype.join
  >;
  export const TypedArrayPrototypeLastIndexOf: UncurryThis<
    typeof Uint8Array.prototype.lastIndexOf
  >;
  export const TypedArrayPrototypeMap: UncurryThis<
    typeof Uint8Array.prototype.map
  >;
  export const TypedArrayPrototypeReduce: UncurryThis<
    typeof Uint8Array.prototype.reduce
  >;
  export const TypedArrayPrototypeReduceRight: UncurryThis<
    typeof Uint8Array.prototype.reduceRight
  >;
  export const TypedArrayPrototypeReverse: UncurryThis<
    typeof Uint8Array.prototype.reverse
  >;
  export const TypedArrayPrototypeSet: UncurryThis<
    typeof Uint8Array.prototype.set
  >;
  export const TypedArrayPrototypeSlice: UncurryThis<
    typeof Uint8Array.prototype.slice
  >;
  export const TypedArrayPrototypeSome: UncurryThis<
    typeof Uint8Array.prototype.some
  >;
  export const TypedArrayPrototypeSort: UncurryThis<
    typeof Uint8Array.prototype.sort
  >;
  export const TypedArrayPrototypeSubarray: UncurryThis<
    typeof Uint8Array.prototype.subarray
  >;
  export const TypedArrayPrototypeToLocaleString: UncurryThis<
    typeof Uint8Array.prototype.toLocaleString
  >;
  export const TypedArrayPrototypeToString: UncurryThis<
    typeof Uint8Array.prototype.toString
  >;
  export const TypedArrayPrototypeValueOf: UncurryThis<
    typeof Uint8Array.prototype.valueOf
  >;
}

declare namespace Deno {
  namespace core {
    /** Mark following promise as "ref", ie. event loop won't exit
     * until all "ref" promises are resolved. All async ops are "ref" by default. */
    function refOpPromise<T>(promise: Promise<T>): void;

    /** Mark following promise as "unref", ie. event loop will exit
     * if there are only "unref" promises left. */
    function unrefOpPromise<T>(promise: Promise<T>): void;

    /**
     * Enables collection of stack traces for sanitizers. This allows for
     * debugging of where a given async op was started. Deno CLI uses this for
     * improving error message in sanitizer errors for `deno test`.
     *
     * **NOTE:** enabling tracing has a significant negative performance impact.
     */
    function setLeakTracingEnabled(enabled: boolean);

    function isLeakTracingEnabled(): boolean;

    /**
     * Returns the origin stack trace of the given async op promise. The promise
     * must be ongoing.
     */
    function getLeakTraceForPromise<T>(promise: Promise<T>): string | null;

    /**
     * Returns a map containing traces for all ongoing async ops. The key is the promise id.
     * Tracing only occurs when `Deno.core.setLeakTracingEnabled()` was previously
     * enabled.
     */
    function getAllLeakTraces(): Map<number, string>;

    /**
     * List of all registered ops, in the form of a map that maps op
     * name to function.
     */
    const ops: Record<string, (...args: unknown[]) => any>;

    /**
     * Retrieve a list of all open resources, in the form of a map that maps
     * resource id to the resource name.
     */
    function resources(): Record<string, string>;

    /**
     * Close the resource with the specified op id. Throws `BadResource` error
     * if resource doesn't exist in resource table.
     */
    function close(rid: number): void;

    /**
     * Try close the resource with the specified op id; if resource with given
     * id doesn't exist do nothing.
     */
    function tryClose(rid: number): void;

    /**
     * Read from a (stream) resource that implements read()
     */
    function read(rid: number, buf: Uint8Array): Promise<number>;

    /**
     * Write to a (stream) resource that implements write()
     */
    function write(rid: number, buf: Uint8Array): Promise<number>;

    /**
     * Write to a (stream) resource that implements write()
     */
    function writeAll(rid: number, buf: Uint8Array): Promise<void>;

    /**
     * Synchronously read from a (stream) resource that implements readSync().
     */
    function readSync(rid: number, buf: Uint8Array): number;

    /**
     * Synchronously write to a (stream) resource that implements writeSync().
     */
    function writeSync(rid: number, buf: Uint8Array): number;

    /**
     * Print a message to stdout or stderr
     */
    function print(message: string, is_err?: boolean): void;

    /**
     * Returns whether the given (file-like) resource is a TTY.
     */
    function isTerminal(rid: number): boolean;

    /**
     * Shutdown a resource
     */
    function shutdown(rid: number): Promise<void>;

    /** Encode a string to its Uint8Array representation. */
    function encode(input: string): Uint8Array;

    /** Decode a string from its Uint8Array representation. */
    function decode(input: Uint8Array): string;

    /**
     * Set a callback that will be called when the WebAssembly streaming APIs
     * (`WebAssembly.compileStreaming` and `WebAssembly.instantiateStreaming`)
     * are called in order to feed the source's bytes to the wasm compiler.
     * The callback is called with the source argument passed to the streaming
     * APIs and an rid to use with the wasm streaming ops.
     *
     * The callback should eventually invoke the following ops:
     *   - `op_wasm_streaming_feed`. Feeds bytes from the wasm resource to the
     *     compiler. Takes the rid and a `Uint8Array`.
     *   - `op_wasm_streaming_abort`. Aborts the wasm compilation. Takes the rid
     *     and an exception. Invalidates the resource.
     *   - `op_wasm_streaming_set_url`. Sets a source URL for the wasm module.
     *     Takes the rid and a string.
     *   - To indicate the end of the resource, use `Deno.core.close()` with the
     *     rid.
     */
    function setWasmStreamingCallback(
      cb: (source: any, rid: number) => void,
    ): void;

    /**
     * Set a callback that will be called after resolving ops and before resolving
     * macrotasks.
     */
    function setNextTickCallback(
      cb: () => void,
    ): void;

    /** Check if there's a scheduled "next tick". */
    function hasNextTickScheduled(): boolean;

    /** Set a value telling the runtime if there are "next ticks" scheduled */
    function setHasNextTickScheduled(value: boolean): void;

    /** Enqueue an immediate callback. Immediate callbacks always execute in
     * the next timer phase.
     */
    function queueImmediate(
      depth: number,
      repeat: boolean,
      delay: number,
      callback: () => void,
    ): number;

    /** Enqueue a user timer at the given depth, optionally repeating. User
     * timers may generate call traces for sanitization, and may be clamped
     * depending on the depth of nesting. */
    function queueUserTimer(
      depth: number,
      repeat: boolean,
      delay: number,
      callback: () => void,
    ): number;

    /** Enqueue a system timer at the given depth, optionally repeating. System
     * timers do not generate call traces, and are never clamped at any nesting
     * depth. System timers are also associated with an op to provide contextual
     * information. */
    function queueSystemTimer(
      associatedOp: Function,
      repeat: boolean,
      delay: number,
      callback: () => void,
    ): number;

    /** Cancel a timer with a given ID. */
    function cancelTimer(id: number);

    /** Ref a timer with a given ID, blocking the runtime from exiting if the timer is still running. */
    function refTimer(id: number);

    /** Unref a timer with a given ID, allowing the runtime to exit if the timer is still running. */
    function unrefTimer(id: number);

    /** Gets the current timer depth. */
    function getTimerDepth(): number;

    /**
     * Set a callback that will be called after resolving ops and "next ticks".
     */
    function setMacrotaskCallback(
      cb: () => boolean,
    ): void;

    /**
     * Sets the unhandled promise rejection handler. The handler returns 'true' if the
     * rejection has been handled. If the handler returns 'false', the promise is considered
     * unhandled, and the runtime then raises an uncatchable error and halts.
     */
    function setUnhandledPromiseRejectionHandler(
      cb: PromiseRejectCallback,
    ): PromiseRejectCallback;

    export type PromiseRejectCallback = (
      promise: Promise<unknown>,
      reason: any,
    ) => boolean;

    /**
     * Sets the handled promise rejection handler.
     */
    function setHandledPromiseRejectionHandler(
      cb: PromiseHandledCallback,
    ): PromiseHandledCallback;

    export type PromiseHandledCallback = (
      promise: Promise<unknown>,
      reason: any,
    ) => void;

    /**
     * Report an exception that was not handled by any runtime handler, and escaped to the
     * top level. This terminates the runtime.
     */
    function reportUnhandledException(e: Error): void;

    /**
     * Report an unhandled promise rejection that was not handled by any runtime handler, and
     * escaped to the top level. This terminates the runtime.
     */
    function reportUnhandledPromiseRejection(e: Error): void;

    /**
     * Set a callback that will be called when an exception isn't caught
     * by any try/catch handlers. Currently only invoked when the callback
     * to setPromiseRejectCallback() throws an exception but that is expected
     * to change in the future. Returns the old handler or undefined.
     */
    function setUncaughtExceptionCallback(
      cb: UncaughtExceptionCallback,
    ): undefined | UncaughtExceptionCallback;

    export type UncaughtExceptionCallback = (err: any) => void;

    export class BadResource extends Error {}
    export const BadResourcePrototype: typeof BadResource.prototype;
    export class Interrupted extends Error {}
    export const InterruptedPrototype: typeof Interrupted.prototype;

    function serialize(
      value: any,
      options?: any,
      errorCallback?,
    ): Uint8Array;

    function deserialize(buffer: Uint8Array, options?: any): any;

    /**
     * Adds a callback for the given Promise event. If this function is called
     * multiple times, the callbacks are called in the order they were added.
     * - `init_hook` is called when a new promise is created. When a new promise
     *   is created as part of the chain in the case of `Promise.then` or in the
     *   intermediate promises created by `Promise.{race, all}`/`AsyncFunctionAwait`,
     *   we pass the parent promise otherwise we pass undefined.
     * - `before_hook` is called at the beginning of the promise reaction.
     * - `after_hook` is called at the end of the promise reaction.
     * - `resolve_hook` is called at the beginning of resolve or reject function.
     */
    function setPromiseHooks(
      init_hook?: (
        promise: Promise<unknown>,
        parentPromise?: Promise<unknown>,
      ) => void,
      before_hook?: (promise: Promise<unknown>) => void,
      after_hook?: (promise: Promise<unknown>) => void,
      resolve_hook?: (promise: Promise<unknown>) => void,
    ): void;

    function isAnyArrayBuffer(
      value: unknown,
    ): value is ArrayBuffer | SharedArrayBuffer;
    function isArgumentsObject(value: unknown): value is IArguments;
    function isArrayBuffer(value: unknown): value is ArrayBuffer;
    function isArrayBufferView(value: unknown): value is ArrayBufferView;
    function isAsyncFunction(
      value: unknown,
    ): value is (
      ...args: unknown[]
    ) => Promise<unknown> | AsyncGeneratorFunction;
    function isBigIntObject(value: unknown): value is BigInt;
    function isBooleanObject(value: unknown): value is Boolean;
    function isBoxedPrimitive(
      value: unknown,
    ): value is BigInt | Boolean | Number | String | Symbol;
    function isDataView(value: unknown): value is DataView;
    function isDate(value: unknown): value is Date;
    function isGeneratorFunction(
      value: unknown,
    ): value is GeneratorFunction | AsyncGeneratorFunction;
    function isGeneratorObject(value: unknown): value is Generator;
    function isMap(value: unknown): value is Map<unknown, unknown>;
    function isMapIterator(value: unknown): value is IterableIterator<unknown>;
    function isModuleNamespaceObject(value: unknown): value is object;
    function isNativeError(value: unknown): value is Error;
    function isNumberObject(value: unknown): value is Number;
    function isPromise(value: unknown): value is Promise<unknown>;
    function isProxy(value: unknown): value is object;
    function isRegExp(value: unknown): value is RegExp;
    function isSet(value: unknown): value is Set<unknown>;
    function isSetIterator(value: unknown): value is IterableIterator<unknown>;
    function isSharedArrayBuffer(value: unknown): value is SharedArrayBuffer;
    function isStringObject(value: unknown): value is String;
    function isSymbolObject(value: unknown): value is Symbol;
    function isTypedArray(
      value: unknown,
    ): value is
      | Uint8Array
      | Uint8ClampedArray
      | Uint16Array
      | Uint32Array
      | Int8Array
      | Int16Array
      | Int32Array
      | Float32Array
      | Float64Array
      | BigUint64Array
      | BigInt64Array;
    function isWeakMap(value: unknown): value is WeakMap<WeakKey, unknown>;
    function isWeakSet(value: unknown): value is WeakSet<WeakKey>;

    function propWritable(value: unknown): PropertyDescriptor;
    function propNonEnumerable(value: unknown): PropertyDescriptor;
    function propReadOnly(value: unknown): PropertyDescriptor;
    function propGetterOnly(value: unknown): PropertyDescriptor;

    function propWritableLazyLoaded<T>(
      getter: (loadedValue: T) => unknown,
      loadFn: LazyLoader<T>,
    ): PropertyDescriptor;
    function propNonEnumerableLazyLoaded<T>(
      getter: (loadedValue: T) => unknown,
      loadFn: LazyLoader<T>,
    ): PropertyDescriptor;

    type LazyLoader<T> = () => T;
    function createLazyLoader<T = unknown>(specifier: string): LazyLoader<T>;

    function createCancelHandle(): number;

    function encodeBinaryString(data: Uint8Array): string;

    const build: {
      target: string;
      arch: string;
      os: string;
      vendor: string;
      env: string | undefined;
    };
  }
}

export declare const core: typeof Deno.core;
