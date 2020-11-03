// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
export { promisify } from "./_util/_util_promisify.ts";
export { callbackify } from "./_util/_util_callbackify.ts";
import { codes, errorMap } from "./_errors.ts";
import * as types from "./_util/_util_types.ts";
export { types };

const NumberIsSafeInteger = Number.isSafeInteger;
const {
  ERR_OUT_OF_RANGE,
  ERR_INVALID_ARG_TYPE,
} = codes;

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

export function isArray(value: unknown): boolean {
  return Array.isArray(value);
}

export function isBoolean(value: unknown): boolean {
  return typeof value === "boolean" || value instanceof Boolean;
}

export function isNull(value: unknown): boolean {
  return value === null;
}

export function isNullOrUndefined(value: unknown): boolean {
  return value === null || value === undefined;
}

export function isNumber(value: unknown): boolean {
  return typeof value === "number" || value instanceof Number;
}

export function isString(value: unknown): boolean {
  return typeof value === "string" || value instanceof String;
}

export function isSymbol(value: unknown): boolean {
  return typeof value === "symbol";
}

export function isUndefined(value: unknown): boolean {
  return value === undefined;
}

export function isObject(value: unknown): boolean {
  return value !== null && typeof value === "object";
}

export function isError(e: unknown): boolean {
  return e instanceof Error;
}

export function isFunction(value: unknown): boolean {
  return typeof value === "function";
}

export function isRegExp(value: unknown): boolean {
  return value instanceof RegExp;
}

export function isPrimitive(value: unknown): boolean {
  return (
    value === null || (typeof value !== "object" && typeof value !== "function")
  );
}

export function getSystemErrorName(code: number): string | undefined {
  if (typeof code !== "number") {
    throw new ERR_INVALID_ARG_TYPE("err", "number", code);
  }
  if (code >= 0 || !NumberIsSafeInteger(code)) {
    throw new ERR_OUT_OF_RANGE("err", "a negative integer", code);
  }
  return errorMap.get(code)?.[0];
}

import { _TextDecoder, _TextEncoder } from "./_utils.ts";

/** The global TextDecoder */
export type TextDecoder = import("./_utils.ts")._TextDecoder;
export const TextDecoder = _TextDecoder;

/** The global TextEncoder */
export type TextEncoder = import("./_utils.ts")._TextEncoder;
export const TextEncoder = _TextEncoder;
