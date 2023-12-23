// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
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

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
const {
  ArrayBufferIsView,
  TypedArrayPrototypeGetSymbolToStringTag,
} = primordials;

export function isAnyArrayBuffer(
  value: unknown,
): value is ArrayBuffer | SharedArrayBuffer {
  return ops.op_is_any_array_buffer(value);
}

export function isArgumentsObject(value: unknown): value is IArguments {
  return ops.op_is_arguments_object(value);
}

export function isArrayBuffer(value: unknown): value is ArrayBuffer {
  return ops.op_is_array_buffer(value);
}

export function isAsyncFunction(
  value: unknown,
): value is (...args: unknown[]) => Promise<unknown> {
  return ops.op_is_async_function(value);
}

// deno-lint-ignore ban-types
export function isBooleanObject(value: unknown): value is Boolean {
  return ops.op_is_boolean_object(value);
}

export function isBoxedPrimitive(
  value: unknown,
  // deno-lint-ignore ban-types
): value is Boolean | String | Number | Symbol | BigInt {
  return ops.op_is_boxed_primitive(value);
}

export function isDataView(value: unknown): value is DataView {
  return (
    ArrayBufferIsView(value) &&
    TypedArrayPrototypeGetSymbolToStringTag(value) === undefined
  );
}

export function isDate(value: unknown): value is Date {
  return ops.op_is_date(value);
}

export function isGeneratorFunction(
  value: unknown,
): value is GeneratorFunction {
  return ops.op_is_generator_function(value);
}

export function isGeneratorObject(value: unknown): value is Generator {
  return ops.op_is_generator_object(value);
}

export function isMap(value: unknown): value is Map<unknown, unknown> {
  return ops.op_is_map(value);
}

export function isMapIterator(
  value: unknown,
): value is IterableIterator<[unknown, unknown]> {
  return ops.op_is_map_iterator(value);
}

export function isModuleNamespaceObject(
  value: unknown,
): value is Record<string | number | symbol, unknown> {
  return ops.op_is_module_namespace_object(value);
}

export function isNativeError(value: unknown): value is Error {
  return ops.op_is_native_error(value);
}

// deno-lint-ignore ban-types
export function isNumberObject(value: unknown): value is Number {
  return ops.op_is_number_object(value);
}

export function isBigIntObject(value: unknown): value is bigint {
  return ops.op_is_big_int_object(value);
}

export function isPromise(value: unknown): value is Promise<unknown> {
  return ops.op_is_promise(value);
}

export function isProxy(
  value: unknown,
): value is Record<string | number | symbol, unknown> {
  return core.isProxy(value);
}

export function isRegExp(value: unknown): value is RegExp {
  return ops.op_is_reg_exp(value);
}

export function isSet(value: unknown): value is Set<unknown> {
  return ops.op_is_set(value);
}

export function isSetIterator(
  value: unknown,
): value is IterableIterator<unknown> {
  return ops.op_is_set_iterator(value);
}

export function isSharedArrayBuffer(
  value: unknown,
): value is SharedArrayBuffer {
  return ops.op_is_shared_array_buffer(value);
}

// deno-lint-ignore ban-types
export function isStringObject(value: unknown): value is String {
  return ops.op_is_string_object(value);
}

// deno-lint-ignore ban-types
export function isSymbolObject(value: unknown): value is Symbol {
  return ops.op_is_symbol_object(value);
}

export function isWeakMap(
  value: unknown,
): value is WeakMap<Record<string | number | symbol, unknown>, unknown> {
  return ops.op_is_weak_map(value);
}

export function isWeakSet(
  value: unknown,
): value is WeakSet<Record<string | number | symbol, unknown>> {
  return ops.op_is_weak_set(value);
}

export default {
  isAsyncFunction,
  isGeneratorFunction,
  isAnyArrayBuffer,
  isArrayBuffer,
  isArgumentsObject,
  isBoxedPrimitive,
  isDataView,
  // isExternal,
  isMap,
  isMapIterator,
  isModuleNamespaceObject,
  isNativeError,
  isPromise,
  isSet,
  isSetIterator,
  isWeakMap,
  isWeakSet,
  isRegExp,
  isDate,
  isStringObject,
  isNumberObject,
  isBooleanObject,
  isBigIntObject,
};
