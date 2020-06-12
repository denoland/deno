// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.
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
import { assertStrictEquals } from "../../testing/asserts.ts";
import {
  isDate,
  isMap,
  isSet,
  isAnyArrayBuffer,
  isArrayBufferView,
  isArgumentsObject,
  isArrayBuffer,
  isStringObject,
  isAsyncFunction,
  isBigInt64Array,
  isBigUint64Array,
  isBooleanObject,
  isBoxedPrimitive,
  isDataView,
  isFloat32Array,
  isFloat64Array,
  isGeneratorFunction,
  isGeneratorObject,
  isInt8Array,
  isInt16Array,
  isInt32Array,
  isMapIterator,
  isModuleNamespaceObject,
  isNativeError,
  isSymbolObject,
  isTypedArray,
  isUint8Array,
  isUint8ClampedArray,
  isUint16Array,
  isUint32Array,
  isNumberObject,
  isBigIntObject,
  isPromise,
  isRegExp,
  isSetIterator,
  isSharedArrayBuffer,
  isWeakMap,
  isWeakSet,
} from "./_util_types.ts";

// Used to test isModuleNamespaceObject
import * as testModuleNamespaceOpbject from "./_util_types.ts";

// isAnyArrayBuffer
Deno.test("Should return true for valid ArrayBuffer types", () => {
  assertStrictEquals(isAnyArrayBuffer(new ArrayBuffer(0)), true);
  assertStrictEquals(isAnyArrayBuffer(new SharedArrayBuffer(0)), true);
});

Deno.test("Should return false for invalid ArrayBuffer types", () => {
  assertStrictEquals(isAnyArrayBuffer({}), false);
  assertStrictEquals(isAnyArrayBuffer([]), false);
  assertStrictEquals(isAnyArrayBuffer(new Error()), false);
});

// isArrayBufferView
Deno.test("Should return true for valid ArrayBufferView types", () => {
  assertStrictEquals(isArrayBufferView(new Int8Array(0)), true);
  assertStrictEquals(isArrayBufferView(new Uint8Array(0)), true);
  assertStrictEquals(isArrayBufferView(new Uint8ClampedArray(0)), true);
  assertStrictEquals(isArrayBufferView(new Int16Array(0)), true);
  assertStrictEquals(isArrayBufferView(new Uint16Array(0)), true);
  assertStrictEquals(isArrayBufferView(new Int32Array(0)), true);
  assertStrictEquals(isArrayBufferView(new Uint32Array(0)), true);
  assertStrictEquals(isArrayBufferView(new Float32Array(0)), true);
  assertStrictEquals(isArrayBufferView(new Float64Array(0)), true);
  assertStrictEquals(isArrayBufferView(new DataView(new ArrayBuffer(0))), true);
});

Deno.test("Should return false for invalid ArrayBufferView types", () => {
  assertStrictEquals(isArrayBufferView({}), false);
  assertStrictEquals(isArrayBufferView([]), false);
  assertStrictEquals(isArrayBufferView(new Error()), false);
  assertStrictEquals(isArrayBufferView(new ArrayBuffer(0)), false);
});

// isArgumentsObject
// Note: not testable in TS

Deno.test("Should return false for invalid Argument types", () => {
  assertStrictEquals(isArgumentsObject({}), false);
  assertStrictEquals(isArgumentsObject([]), false);
  assertStrictEquals(isArgumentsObject(new Error()), false);
});

// isArrayBuffer
Deno.test("Should return true for valid ArrayBuffer types", () => {
  assertStrictEquals(isArrayBuffer(new ArrayBuffer(0)), true);
});

Deno.test("Should return false for invalid ArrayBuffer types", () => {
  assertStrictEquals(isArrayBuffer(new SharedArrayBuffer(0)), false);
  assertStrictEquals(isArrayBuffer({}), false);
  assertStrictEquals(isArrayBuffer([]), false);
  assertStrictEquals(isArrayBuffer(new Error()), false);
});

// isAsyncFunction
Deno.test("Should return true for valid async function types", () => {
  const asyncFunction = async (): Promise<void> => {};
  assertStrictEquals(isAsyncFunction(asyncFunction), true);
});

Deno.test("Should return false for invalid async function types", () => {
  const syncFunction = (): void => {};
  assertStrictEquals(isAsyncFunction(syncFunction), false);
  assertStrictEquals(isAsyncFunction({}), false);
  assertStrictEquals(isAsyncFunction([]), false);
  assertStrictEquals(isAsyncFunction(new Error()), false);
});

// isBigInt64Array
Deno.test("Should return true for valid BigInt64Array types", () => {
  assertStrictEquals(isBigInt64Array(new BigInt64Array()), true);
});

Deno.test("Should return false for invalid BigInt64Array types", () => {
  assertStrictEquals(isBigInt64Array(new BigUint64Array()), false);
  assertStrictEquals(isBigInt64Array(new Float32Array()), false);
  assertStrictEquals(isBigInt64Array(new Int32Array()), false);
});

// isBigUint64Array
Deno.test("Should return true for valid isBigUint64Array types", () => {
  assertStrictEquals(isBigUint64Array(new BigUint64Array()), true);
});

Deno.test("Should return false for invalid isBigUint64Array types", () => {
  assertStrictEquals(isBigUint64Array(new BigInt64Array()), false);
  assertStrictEquals(isBigUint64Array(new Float32Array()), false);
  assertStrictEquals(isBigUint64Array(new Int32Array()), false);
});

// isBooleanObject
Deno.test("Should return true for valid Boolean object types", () => {
  assertStrictEquals(isBooleanObject(new Boolean(false)), true);
  assertStrictEquals(isBooleanObject(new Boolean(true)), true);
});

Deno.test("Should return false for invalid isBigUint64Array types", () => {
  assertStrictEquals(isBooleanObject(false), false);
  assertStrictEquals(isBooleanObject(true), false);
  assertStrictEquals(isBooleanObject(Boolean(false)), false);
  assertStrictEquals(isBooleanObject(Boolean(true)), false);
});

// isBoxedPrimitive
Deno.test("Should return true for valid boxed primitive values", () => {
  assertStrictEquals(isBoxedPrimitive(new Boolean(false)), true);
  assertStrictEquals(isBoxedPrimitive(Object(Symbol("foo"))), true);
  assertStrictEquals(isBoxedPrimitive(Object(BigInt(5))), true);
  assertStrictEquals(isBoxedPrimitive(new String("foo")), true);
});

Deno.test("Should return false for invalid boxed primitive values", () => {
  assertStrictEquals(isBoxedPrimitive(false), false);
  assertStrictEquals(isBoxedPrimitive(Symbol("foo")), false);
});

// isDateView
Deno.test("Should return true for valid DataView types", () => {
  assertStrictEquals(isDataView(new DataView(new ArrayBuffer(0))), true);
});

Deno.test("Should return false for invalid DataView types", () => {
  assertStrictEquals(isDataView(new Float64Array(0)), false);
});

// isDate
Deno.test("Should return true for valid date types", () => {
  assertStrictEquals(isDate(new Date()), true);
  assertStrictEquals(isDate(new Date(0)), true);
  assertStrictEquals(isDate(new (eval("Date"))()), true);
});

Deno.test("Should return false for invalid date types", () => {
  assertStrictEquals(isDate(Date()), false);
  assertStrictEquals(isDate({}), false);
  assertStrictEquals(isDate([]), false);
  assertStrictEquals(isDate(new Error()), false);
  assertStrictEquals(isDate(Object.create(Date.prototype)), false);
});

// isFloat32Array
Deno.test("Should return true for valid Float32Array types", () => {
  assertStrictEquals(isFloat32Array(new Float32Array(0)), true);
});

Deno.test("Should return false for invalid Float32Array types", () => {
  assertStrictEquals(isFloat32Array(new ArrayBuffer(0)), false);
  assertStrictEquals(isFloat32Array(new Float64Array(0)), false);
});

// isFloat64Array
Deno.test("Should return true for valid Float64Array types", () => {
  assertStrictEquals(isFloat64Array(new Float64Array(0)), true);
});

Deno.test("Should return false for invalid Float64Array types", () => {
  assertStrictEquals(isFloat64Array(new ArrayBuffer(0)), false);
  assertStrictEquals(isFloat64Array(new Uint8Array(0)), false);
});

// isGeneratorFunction
Deno.test("Should return true for valid generator functions", () => {
  assertStrictEquals(
    isGeneratorFunction(function* foo() {}),
    true
  );
});

Deno.test("Should return false for invalid generator functions", () => {
  assertStrictEquals(
    isGeneratorFunction(function foo() {}),
    false
  );
});

// isGeneratorObject
Deno.test("Should return true for valid generator object types", () => {
  function* foo(): Iterator<void> {}
  assertStrictEquals(isGeneratorObject(foo()), true);
});

Deno.test("Should return false for invalid generation object types", () => {
  assertStrictEquals(
    isGeneratorObject(function* foo() {}),
    false
  );
});

// isInt8Array
Deno.test("Should return true for valid Int8Array types", () => {
  assertStrictEquals(isInt8Array(new Int8Array(0)), true);
});

Deno.test("Should return false for invalid Int8Array types", () => {
  assertStrictEquals(isInt8Array(new ArrayBuffer(0)), false);
  assertStrictEquals(isInt8Array(new Float64Array(0)), false);
});

// isInt16Array
Deno.test("Should return true for valid Int16Array types", () => {
  assertStrictEquals(isInt16Array(new Int16Array(0)), true);
});

Deno.test("Should return false for invalid Int16Array type", () => {
  assertStrictEquals(isInt16Array(new ArrayBuffer(0)), false);
  assertStrictEquals(isInt16Array(new Float64Array(0)), false);
});

// isInt32Array
Deno.test("Should return true for valid isInt32Array types", () => {
  assertStrictEquals(isInt32Array(new Int32Array(0)), true);
});

Deno.test("Should return false for invalid isInt32Array type", () => {
  assertStrictEquals(isInt32Array(new ArrayBuffer(0)), false);
  assertStrictEquals(isInt32Array(new Float64Array(0)), false);
});

// isStringObject
Deno.test("Should return true for valid String types", () => {
  assertStrictEquals(isStringObject(new String("")), true);
  assertStrictEquals(isStringObject(new String("Foo")), true);
});

Deno.test("Should return false for invalid String types", () => {
  assertStrictEquals(isStringObject(""), false);
  assertStrictEquals(isStringObject("Foo"), false);
});

// isMap
Deno.test("Should return true for valid Map types", () => {
  assertStrictEquals(isMap(new Map()), true);
});

Deno.test("Should return false for invalid Map types", () => {
  assertStrictEquals(isMap({}), false);
  assertStrictEquals(isMap([]), false);
  assertStrictEquals(isMap(new Date()), false);
  assertStrictEquals(isMap(new Error()), false);
});

// isMapIterator
Deno.test("Should return true for valid Map Iterator types", () => {
  const map = new Map();
  assertStrictEquals(isMapIterator(map.keys()), true);
  assertStrictEquals(isMapIterator(map.values()), true);
  assertStrictEquals(isMapIterator(map.entries()), true);
  assertStrictEquals(isMapIterator(map[Symbol.iterator]()), true);
});

Deno.test("Should return false for invalid Map iterator types", () => {
  assertStrictEquals(isMapIterator(new Map()), false);
  assertStrictEquals(isMapIterator([]), false);
  assertStrictEquals(isMapIterator(new Date()), false);
  assertStrictEquals(isMapIterator(new Error()), false);
});

// isModuleNamespaceObject
Deno.test("Should return true for valid module namespace objects", () => {
  assertStrictEquals(isModuleNamespaceObject(testModuleNamespaceOpbject), true);
});

Deno.test("Should return false for invalid  module namespace objects", () => {
  assertStrictEquals(isModuleNamespaceObject(assertStrictEquals), false);
});

// isNativeError
Deno.test("Should return true for valid Error types", () => {
  assertStrictEquals(isNativeError(new Error()), true);
  assertStrictEquals(isNativeError(new TypeError()), true);
  assertStrictEquals(isNativeError(new RangeError()), true);
});

Deno.test("Should return false for invalid Error types", () => {
  assertStrictEquals(isNativeError(null), false);
  assertStrictEquals(isNativeError(NaN), false);
});

// isNumberObject
Deno.test("Should return true for valid number objects", () => {
  assertStrictEquals(isNumberObject(new Number(0)), true);
});

Deno.test("Should return false for invalid number types", () => {
  assertStrictEquals(isNumberObject(0), false);
});

// isBigIntObject
Deno.test("Should return true for valid number objects", () => {
  assertStrictEquals(isBigIntObject(new Object(BigInt(42))), true);
});

Deno.test("Should return false for invalid number types", () => {
  assertStrictEquals(isBigIntObject(BigInt(42)), false);
});

// isPromise
Deno.test("Should return true for valid Promise types", () => {
  assertStrictEquals(isPromise(Promise.resolve(42)), true);
});

Deno.test("Should return false for invalid Promise types", () => {
  assertStrictEquals(isPromise(new Object()), false);
});

// isRegExp
Deno.test("Should return true for valid RegExp", () => {
  assertStrictEquals(isRegExp(/abc/), true);
  assertStrictEquals(isRegExp(new RegExp("abc")), true);
});

Deno.test("Should return false for invalid RegExp types", () => {
  assertStrictEquals(isRegExp({}), false);
  assertStrictEquals(isRegExp("/abc/"), false);
});

// isSet
Deno.test("Should return true for valid Set types", () => {
  assertStrictEquals(isSet(new Set()), true);
});

Deno.test("Should return false for invalid Set types", () => {
  assertStrictEquals(isSet({}), false);
  assertStrictEquals(isSet([]), false);
  assertStrictEquals(isSet(new Map()), false);
  assertStrictEquals(isSet(new Error()), false);
});

// isSetIterator
Deno.test("Should return true for valid Set Iterator types", () => {
  const set = new Set();
  assertStrictEquals(isSetIterator(set.keys()), true);
  assertStrictEquals(isSetIterator(set.values()), true);
  assertStrictEquals(isSetIterator(set.entries()), true);
  assertStrictEquals(isSetIterator(set[Symbol.iterator]()), true);
});

Deno.test("Should return false for invalid Set Iterator types", () => {
  assertStrictEquals(isSetIterator(new Set()), false);
  assertStrictEquals(isSetIterator([]), false);
  assertStrictEquals(isSetIterator(new Map()), false);
  assertStrictEquals(isSetIterator(new Error()), false);
});

// isSharedArrayBuffer
Deno.test("Should return true for valid SharedArrayBuffer types", () => {
  assertStrictEquals(isSharedArrayBuffer(new SharedArrayBuffer(0)), true);
});

Deno.test("Should return false for invalid SharedArrayBuffer types", () => {
  assertStrictEquals(isSharedArrayBuffer(new ArrayBuffer(0)), false);
});

// isStringObject
Deno.test("Should return true for valid String Object types", () => {
  assertStrictEquals(isStringObject(new String("")), true);
  assertStrictEquals(isStringObject(new String("Foo")), true);
});

Deno.test("Should return false for invalid String Object types", () => {
  assertStrictEquals(isStringObject(""), false);
  assertStrictEquals(isStringObject("Foo"), false);
});

// isSymbolObject
Deno.test("Should return true for valid Symbol types", () => {
  assertStrictEquals(isSymbolObject(Object(Symbol("foo"))), true);
});

Deno.test("Should return false for invalid Symbol types", () => {
  assertStrictEquals(isSymbolObject(Symbol("foo")), false);
});

// isTypedArray
Deno.test("Should return true for valid TypedArray types", () => {
  assertStrictEquals(isTypedArray(new Uint8Array(0)), true);
  assertStrictEquals(isTypedArray(new Float64Array(0)), true);
});

Deno.test("Should return false for invalid TypedArray types", () => {
  assertStrictEquals(isTypedArray(new ArrayBuffer(0)), false);
});

// isUint8Array
Deno.test("Should return true for valid Uint8Array types", () => {
  assertStrictEquals(isUint8Array(new Uint8Array(0)), true);
});

Deno.test("Should return false for invalid Uint8Array types", () => {
  assertStrictEquals(isUint8Array(new ArrayBuffer(0)), false);
  assertStrictEquals(isUint8Array(new Float64Array(0)), false);
});

// isUint8ClampedArray
Deno.test("Should return true for valid Uint8ClampedArray types", () => {
  assertStrictEquals(isUint8ClampedArray(new Uint8ClampedArray(0)), true);
});

Deno.test("Should return false for invalid Uint8Array types", () => {
  assertStrictEquals(isUint8ClampedArray(new ArrayBuffer(0)), false);
  assertStrictEquals(isUint8ClampedArray(new Float64Array(0)), false);
});

// isUint16Array
Deno.test("Should return true for valid isUint16Array types", () => {
  assertStrictEquals(isUint16Array(new Uint16Array(0)), true);
});

Deno.test("Should return false for invalid Uint16Array types", () => {
  assertStrictEquals(isUint16Array(new ArrayBuffer(0)), false);
  assertStrictEquals(isUint16Array(new Float64Array(0)), false);
});

// isUint32Array
Deno.test("Should return true for valid Uint32Array types", () => {
  assertStrictEquals(isUint32Array(new Uint32Array(0)), true);
});

Deno.test("Should return false for invalid isUint16Array types", () => {
  assertStrictEquals(isUint32Array(new ArrayBuffer(0)), false);
  assertStrictEquals(isUint32Array(new Float64Array(0)), false);
});

// isWeakMap
Deno.test("Should return true for valid WeakMap types", () => {
  assertStrictEquals(isWeakMap(new WeakMap()), true);
});

Deno.test("Should return false for invalid WeakMap types", () => {
  assertStrictEquals(isWeakMap(new Set()), false);
  assertStrictEquals(isWeakMap(new Map()), false);
});

// isWeakSet
Deno.test("Should return true for valid WeakSet types", () => {
  assertStrictEquals(isWeakSet(new WeakSet()), true);
});

Deno.test("Should return false for invalid WeakSet types", () => {
  assertStrictEquals(isWeakSet(new Set()), false);
  assertStrictEquals(isWeakSet(new Map()), false);
});
