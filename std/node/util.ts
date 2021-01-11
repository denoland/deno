// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
export { promisify } from "./_util/_util_promisify.ts";
export { callbackify } from "./_util/_util_callbackify.ts";
import { ERR_INVALID_ARG_TYPE, ERR_OUT_OF_RANGE, errorMap } from "./_errors.ts";
import * as types from "./_util/_util_types.ts";
export { types };

const NumberIsSafeInteger = Number.isSafeInteger;

const DEFAULT_INSPECT_OPTIONS = {
  showHidden: false,
  depth: 2,
  colors: false,
  customInspect: true,
  showProxy: false,
  maxArrayLength: 100,
  maxStringLength: Infinity,
  breakLength: 80,
  compact: 3,
  sorted: false,
  getters: false,
};

inspect.defaultOptions = DEFAULT_INSPECT_OPTIONS;
inspect.custom = Deno.customInspect;

// TODO(schwarzkopfb): make it in-line with Node's implementation
// Ref: https://nodejs.org/dist/latest-v14.x/docs/api/util.html#util_util_inspect_object_options
// deno-lint-ignore no-explicit-any
export function inspect(object: unknown, ...opts: any): string {
  opts = { ...DEFAULT_INSPECT_OPTIONS, ...opts };
  return Deno.inspect(object, {
    depth: opts.depth,
    iterableLimit: opts.maxArrayLength,
    compact: !!opts.compact,
    sorted: !!opts.sorted,
    showProxy: !!opts.showProxy,
  });
}

/** @deprecated - use `Array.isArray()` instead. */
export function isArray(value: unknown): boolean {
  return Array.isArray(value);
}

/** @deprecated - use `typeof value === "boolean" || value instanceof Boolean` instead. */
export function isBoolean(value: unknown): boolean {
  return typeof value === "boolean" || value instanceof Boolean;
}

/** @deprecated - use `value === null` instead. */
export function isNull(value: unknown): boolean {
  return value === null;
}

/** @deprecated - use `value === null || value === undefined` instead. */
export function isNullOrUndefined(value: unknown): boolean {
  return value === null || value === undefined;
}

/** @deprecated - use `typeof value === "number" || value instanceof Number` instead. */
export function isNumber(value: unknown): boolean {
  return typeof value === "number" || value instanceof Number;
}

/** @deprecated - use `typeof value === "string" || value instanceof String` instead. */
export function isString(value: unknown): boolean {
  return typeof value === "string" || value instanceof String;
}

/** @deprecated - use `typeof value === "symbol"` instead. */
export function isSymbol(value: unknown): boolean {
  return typeof value === "symbol";
}

/** @deprecated - use `value === undefined` instead. */
export function isUndefined(value: unknown): boolean {
  return value === undefined;
}

/** @deprecated - use `value !== null && typeof value === "object"` instead. */
export function isObject(value: unknown): boolean {
  return value !== null && typeof value === "object";
}

/** @deprecated - use `e instanceof Error` instead. */
export function isError(e: unknown): boolean {
  return e instanceof Error;
}

/** @deprecated - use `typeof value === "function"` instead. */
export function isFunction(value: unknown): boolean {
  return typeof value === "function";
}

/** @deprecated - use `value instanceof RegExp` instead. */
export function isRegExp(value: unknown): boolean {
  return value instanceof RegExp;
}

/** @deprecated - use `value === null || (typeof value !== "object" && typeof value !== "function")` instead. */
export function isPrimitive(value: unknown): boolean {
  return (
    value === null || (typeof value !== "object" && typeof value !== "function")
  );
}

/** 
 * Returns a system error name from an error code number.
 * @param code error code number
 */
export function getSystemErrorName(code: number): string | undefined {
  if (typeof code !== "number") {
    throw new ERR_INVALID_ARG_TYPE("err", "number", code);
  }
  if (code >= 0 || !NumberIsSafeInteger(code)) {
    throw new ERR_OUT_OF_RANGE("err", "a negative integer", code);
  }
  return errorMap.get(code)?.[0];
}

/**
 * https://nodejs.org/api/util.html#util_util_deprecate_fn_msg_code
 * @param _code This implementation of deprecate won't apply the deprecation code
 */
export function deprecate<A extends Array<unknown>, B>(
  this: unknown,
  callback: (...args: A) => B,
  msg: string,
  _code?: string,
) {
  return function (this: unknown, ...args: A) {
    console.warn(msg);
    return callback.apply(this, args);
  };
}

import { _TextDecoder, _TextEncoder } from "./_utils.ts";

/** The global TextDecoder */
export type TextDecoder = import("./_utils.ts")._TextDecoder;
export const TextDecoder = _TextDecoder;

/** The global TextEncoder */
export type TextEncoder = import("./_utils.ts")._TextEncoder;
export const TextEncoder = _TextEncoder;

export default {
  inspect,
  isArray,
  isBoolean,
  isNull,
  isNullOrUndefined,
  isNumber,
  isString,
  isSymbol,
  isUndefined,
  isObject,
  isError,
  isFunction,
  isRegExp,
  isPrimitive,
  getSystemErrorName,
  deprecate,
  TextDecoder,
  TextEncoder,
};
