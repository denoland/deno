// Copyright 2018-2025 the Deno authors. MIT license.
import * as module from "checkin:testing";
import { assert, test } from "checkin:testing";

test(function testIsAnyArrayBuffer() {
  assert(Deno.core.isAnyArrayBuffer(new ArrayBuffer(4)));
  assert(Deno.core.isAnyArrayBuffer(new SharedArrayBuffer(4)));
  assert(!Deno.core.isAnyArrayBuffer(new Uint8Array(4)));
});

test(function testIsArgumentsObject() {
  let args: IArguments;
  (function () {
    args = arguments;
  })();
  assert(Deno.core.isArgumentsObject(args));
  assert(!Deno.core.isArgumentsObject({}));
});

test(function testIsArrayBuffer() {
  assert(Deno.core.isArrayBuffer(new ArrayBuffer(4)));
  assert(!Deno.core.isArrayBuffer(new SharedArrayBuffer(4)));
  assert(!Deno.core.isArrayBuffer(new Uint8Array(4)));
});

test(function testIsArrayBufferView() {
  assert(Deno.core.isArrayBufferView(new DataView(new ArrayBuffer(4))));
  assert(Deno.core.isArrayBufferView(new Uint8Array(4)));
  assert(!Deno.core.isArrayBufferView(new ArrayBuffer(4)));
});

test(function testIsAsyncFunction() {
  assert(Deno.core.isAsyncFunction(async function () {}));
  assert(Deno.core.isAsyncFunction(async function* () {}));
  assert(!Deno.core.isAsyncFunction(function () {}));
  assert(!Deno.core.isAsyncFunction(function* () {}));
});

test(function testIsBigIntObject() {
  assert(Deno.core.isBigIntObject(Object(1n)));
  assert(!Deno.core.isBigIntObject(1n));
  assert(!Deno.core.isBigIntObject(1));
});

test(function testIsBooleanObject() {
  assert(Deno.core.isBooleanObject(new Boolean(true)));
  assert(!Deno.core.isBooleanObject(true));
  assert(!Deno.core.isBooleanObject("true"));
});

test(function testIsBoxedPrimitive() {
  assert(Deno.core.isBoxedPrimitive(Object(1n)));
  assert(Deno.core.isBoxedPrimitive(new Boolean(true)));
  assert(Deno.core.isBoxedPrimitive(new Number(1)));
  assert(Deno.core.isBoxedPrimitive(Object(Symbol())));
  assert(Deno.core.isBoxedPrimitive(new String("str")));
  assert(!Deno.core.isBoxedPrimitive(1n));
  assert(!Deno.core.isBoxedPrimitive(true));
  assert(!Deno.core.isBoxedPrimitive(1));
  assert(!Deno.core.isBoxedPrimitive(Symbol()));
  assert(!Deno.core.isBoxedPrimitive("str"));
});

test(function testIsDataView() {
  assert(Deno.core.isDataView(new DataView(new ArrayBuffer(4))));
  assert(!Deno.core.isDataView(new Uint8Array(4)));
  assert(!Deno.core.isDataView(new ArrayBuffer(4)));
});

test(function testIsDate() {
  assert(Deno.core.isDate(new Date()));
  assert(!Deno.core.isDate({}));
});

test(function testIsGeneratorFunction() {
  assert(Deno.core.isGeneratorFunction(async function* () {}));
  assert(Deno.core.isGeneratorFunction(function* () {}));
  assert(!Deno.core.isGeneratorFunction(async function () {}));
  assert(!Deno.core.isGeneratorFunction(function () {}));
});

test(function testIsGeneratorObject() {
  const generator = (function* () {})();
  assert(Deno.core.isGeneratorObject(generator));
  assert(!Deno.core.isGeneratorObject({}));
});

test(function testIsMap() {
  assert(Deno.core.isMap(new Map()));
  assert(!Deno.core.isMap(new Set()));
  assert(!Deno.core.isMap(new WeakMap()));
  assert(!Deno.core.isMap(new WeakSet()));
});

test(function testIsMapIterator() {
  const map = new Map();
  assert(Deno.core.isMapIterator(map.values()));
  assert(!Deno.core.isMapIterator(map));
});

test(function testIsModuleNamespaceObject() {
  assert(Deno.core.isModuleNamespaceObject(module));
  assert(!Deno.core.isModuleNamespaceObject({}));
});

test(function testIsNativeError() {
  assert(Deno.core.isNativeError(new Error()));
  assert(Deno.core.isNativeError(new TypeError()));
  assert(!Deno.core.isNativeError({}));
});

test(function testIsNativeError() {
  assert(Deno.core.isNumberObject(new Number(1)));
  assert(!Deno.core.isNumberObject(1));
});

test(function testIsPromise() {
  assert(Deno.core.isPromise(new Promise((resolve) => resolve(1))));
  assert(!Deno.core.isPromise({}));
});

test(function testIsProxy() {
  assert(Deno.core.isProxy(new Proxy({}, {})));
  assert(!Deno.core.isProxy({}));
});

test(function testIsRegExp() {
  assert(Deno.core.isRegExp(/foo/));
  assert(!Deno.core.isRegExp({}));
});

test(function testIsSet() {
  assert(Deno.core.isSet(new Set()));
  assert(!Deno.core.isSet(new Map()));
  assert(!Deno.core.isSet(new WeakSet()));
  assert(!Deno.core.isSet(new WeakMap()));
});

test(function testIsSetIterator() {
  const set = new Set();
  assert(Deno.core.isSetIterator(set.values()));
  assert(!Deno.core.isSetIterator(set));
});

test(function testIsSharedArrayBuffer() {
  assert(Deno.core.isSharedArrayBuffer(new SharedArrayBuffer(4)));
  assert(!Deno.core.isSharedArrayBuffer(new ArrayBuffer(4)));
  assert(!Deno.core.isSharedArrayBuffer(new Uint8Array(4)));
});

test(function testIsStringObject() {
  assert(Deno.core.isStringObject(new String("str")));
  assert(!Deno.core.isStringObject("str"));
});

test(function testIsSymbolObject() {
  assert(Deno.core.isSymbolObject(Object(Symbol())));
  assert(!Deno.core.isSymbolObject(Symbol()));
});

test(function testIsTypedArray() {
  assert(Deno.core.isTypedArray(new Uint8Array(4)));
  assert(!Deno.core.isTypedArray(new DataView(new ArrayBuffer(4))));
  assert(!Deno.core.isTypedArray(new ArrayBuffer(4)));
});

test(function testIsWeakMap() {
  assert(Deno.core.isWeakMap(new WeakMap()));
  assert(!Deno.core.isWeakMap(new WeakSet()));
  assert(!Deno.core.isWeakMap(new Map()));
  assert(!Deno.core.isWeakMap(new Set()));
});

test(function testIsWeakSet() {
  assert(Deno.core.isWeakSet(new WeakSet()));
  assert(!Deno.core.isWeakSet(new WeakMap()));
  assert(!Deno.core.isWeakSet(new Set()));
  assert(!Deno.core.isWeakSet(new Map()));
});
