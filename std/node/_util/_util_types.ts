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

const _toString = Object.prototype.toString;

const _isObjectLike = (value: unknown): boolean =>
  value !== null && typeof value === "object";

const _isFunctionLike = (value: unknown): boolean =>
  value !== null && typeof value === "function";

export function isAnyArrayBuffer(value: unknown): boolean {
  return (
    _isObjectLike(value) &&
    (_toString.call(value) === "[object ArrayBuffer]" ||
      _toString.call(value) === "[object SharedArrayBuffer]")
  );
}

export function isArrayBufferView(value: unknown): boolean {
  return ArrayBuffer.isView(value);
}

export function isArgumentsObject(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Arguments]";
}

export function isArrayBuffer(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object ArrayBuffer]"
  );
}

export function isAsyncFunction(value: unknown): boolean {
  return (
    _isFunctionLike(value) && _toString.call(value) === "[object AsyncFunction]"
  );
}

export function isBigInt64Array(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object BigInt64Array]"
  );
}

export function isBigUint64Array(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object BigUint64Array]"
  );
}

export function isBooleanObject(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Boolean]";
}

export function isBoxedPrimitive(value: unknown): boolean {
  return (
    isBooleanObject(value) ||
    isStringObject(value) ||
    isNumberObject(value) ||
    isSymbolObject(value) ||
    isBigIntObject(value)
  );
}

export function isDataView(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object DataView]";
}

export function isDate(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Date]";
}

// isExternal: Not implemented

export function isFloat32Array(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object Float32Array]"
  );
}

export function isFloat64Array(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object Float64Array]"
  );
}

export function isGeneratorFunction(value: unknown): boolean {
  return (
    _isFunctionLike(value) &&
    _toString.call(value) === "[object GeneratorFunction]"
  );
}

export function isGeneratorObject(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Generator]";
}

export function isInt8Array(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Int8Array]";
}

export function isInt16Array(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object Int16Array]"
  );
}

export function isInt32Array(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object Int32Array]"
  );
}

export function isMap(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Map]";
}

export function isMapIterator(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object Map Iterator]"
  );
}

export function isModuleNamespaceObject(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Module]";
}

export function isNativeError(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Error]";
}

export function isNumberObject(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Number]";
}

export function isBigIntObject(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object BigInt]";
}

export function isPromise(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Promise]";
}

export function isRegExp(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object RegExp]";
}

export function isSet(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Set]";
}

export function isSetIterator(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object Set Iterator]"
  );
}

export function isSharedArrayBuffer(value: unknown): boolean {
  return (
    _isObjectLike(value) &&
    _toString.call(value) === "[object SharedArrayBuffer]"
  );
}

export function isStringObject(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object String]";
}

export function isSymbolObject(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object Symbol]";
}

// Adapted from Lodash
export function isTypedArray(value: unknown): boolean {
  /** Used to match `toStringTag` values of typed arrays. */
  const reTypedTag = /^\[object (?:Float(?:32|64)|(?:Int|Uint)(?:8|16|32)|Uint8Clamped)Array\]$/;
  return _isObjectLike(value) && reTypedTag.test(_toString.call(value));
}

export function isUint8Array(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object Uint8Array]"
  );
}

export function isUint8ClampedArray(value: unknown): boolean {
  return (
    _isObjectLike(value) &&
    _toString.call(value) === "[object Uint8ClampedArray]"
  );
}

export function isUint16Array(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object Uint16Array]"
  );
}

export function isUint32Array(value: unknown): boolean {
  return (
    _isObjectLike(value) && _toString.call(value) === "[object Uint32Array]"
  );
}

export function isWeakMap(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object WeakMap]";
}

export function isWeakSet(value: unknown): boolean {
  return _isObjectLike(value) && _toString.call(value) === "[object WeakSet]";
}
