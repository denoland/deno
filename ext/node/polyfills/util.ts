// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { promisify } from "ext:deno_node/internal/util.mjs";
import { callbackify } from "ext:deno_node/_util/_util_callbackify.ts";
import { debuglog } from "ext:deno_node/internal/util/debuglog.ts";
import {
  format,
  formatWithOptions,
  inspect,
  stripVTControlCharacters,
} from "ext:deno_node/internal/util/inspect.mjs";
import { codes } from "ext:deno_node/internal/error_codes.ts";
import types from "ext:deno_node/util/types.ts";
import { Buffer } from "ext:deno_node/buffer.ts";
import { isDeepStrictEqual } from "ext:deno_node/internal/util/comparisons.ts";
import process from "ext:deno_node/process.ts";
import { validateString } from "ext:deno_node/internal/validators.mjs";

export {
  callbackify,
  debuglog,
  format,
  formatWithOptions,
  inspect,
  promisify,
  stripVTControlCharacters,
  types,
};

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

/** @deprecated Use util.types.RegExp() instead. */
export function isRegExp(value: unknown): boolean {
  return types.isRegExp(value);
}

/** @deprecated Use util.types.isDate() instead. */
export function isDate(value: unknown): boolean {
  return types.isDate(value);
}

/** @deprecated - use `value === null || (typeof value !== "object" && typeof value !== "function")` instead. */
export function isPrimitive(value: unknown): boolean {
  return (
    value === null || (typeof value !== "object" && typeof value !== "function")
  );
}

/** @deprecated  Use Buffer.isBuffer() instead. */
export function isBuffer(value: unknown): boolean {
  return Buffer.isBuffer(value);
}

/** @deprecated Use Object.assign() instead. */
export function _extend(
  target: Record<string, unknown>,
  source: unknown,
): Record<string, unknown> {
  // Don't do anything if source isn't an object
  if (source === null || typeof source !== "object") return target;

  const keys = Object.keys(source!);
  let i = keys.length;
  while (i--) {
    target[keys[i]] = (source as Record<string, unknown>)[keys[i]];
  }
  return target;
}

/**
 * https://nodejs.org/api/util.html#util_util_inherits_constructor_superconstructor
 * @param ctor Constructor function which needs to inherit the prototype.
 * @param superCtor Constructor function to inherit prototype from.
 */
export function inherits<T, U>(
  ctor: new (...args: unknown[]) => T,
  superCtor: new (...args: unknown[]) => U,
) {
  if (ctor === undefined || ctor === null) {
    throw new codes.ERR_INVALID_ARG_TYPE("ctor", "Function", ctor);
  }

  if (superCtor === undefined || superCtor === null) {
    throw new codes.ERR_INVALID_ARG_TYPE("superCtor", "Function", superCtor);
  }

  if (superCtor.prototype === undefined) {
    throw new codes.ERR_INVALID_ARG_TYPE(
      "superCtor.prototype",
      "Object",
      superCtor.prototype,
    );
  }
  Object.defineProperty(ctor, "super_", {
    value: superCtor,
    writable: true,
    configurable: true,
  });
  Object.setPrototypeOf(ctor.prototype, superCtor.prototype);
}

import {
  _TextDecoder,
  _TextEncoder,
  getSystemErrorName,
} from "ext:deno_node/_utils.ts";

/** The global TextDecoder */
export type TextDecoder = import("./_utils.ts")._TextDecoder;
export const TextDecoder = _TextDecoder;

/** The global TextEncoder */
export type TextEncoder = import("./_utils.ts")._TextEncoder;
export const TextEncoder = _TextEncoder;

function pad(n: number) {
  return n.toString().padStart(2, "0");
}

const months = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

/**
 * @returns 26 Feb 16:19:34
 */
function timestamp(): string {
  const d = new Date();
  const t = [
    pad(d.getHours()),
    pad(d.getMinutes()),
    pad(d.getSeconds()),
  ].join(":");
  return `${(d.getDate())} ${months[(d).getMonth()]} ${t}`;
}

/**
 * Log is just a thin wrapper to console.log that prepends a timestamp
 * @deprecated
 */
// deno-lint-ignore no-explicit-any
export function log(...args: any[]) {
  console.log("%s - %s", timestamp(), format(...args));
}

// Keep a list of deprecation codes that have been warned on so we only warn on
// each one once.
const codesWarned = new Set();

// Mark that a method should not be used.
// Returns a modified function which warns once by default.
// If --no-deprecation is set, then it is a no-op.
// deno-lint-ignore no-explicit-any
export function deprecate(fn: any, msg: string, code?: any) {
  if (process.noDeprecation === true) {
    return fn;
  }

  if (code !== undefined) {
    validateString(code, "code");
  }

  let warned = false;
  // deno-lint-ignore no-explicit-any
  function deprecated(this: any, ...args: any[]) {
    if (!warned) {
      warned = true;
      if (code !== undefined) {
        if (!codesWarned.has(code)) {
          process.emitWarning(msg, "DeprecationWarning", code, deprecated);
          codesWarned.add(code);
        }
      } else {
        // deno-lint-ignore no-explicit-any
        process.emitWarning(msg, "DeprecationWarning", deprecated as any);
      }
    }
    if (new.target) {
      return Reflect.construct(fn, args, new.target);
    }
    return Reflect.apply(fn, this, args);
  }

  // The wrapper will keep the same prototype as fn to maintain prototype chain
  Object.setPrototypeOf(deprecated, fn);
  if (fn.prototype) {
    // Setting this (rather than using Object.setPrototype, as above) ensures
    // that calling the unwrapped constructor gives an instanceof the wrapped
    // constructor.
    deprecated.prototype = fn.prototype;
  }

  return deprecated;
}

export { getSystemErrorName, isDeepStrictEqual };

export default {
  format,
  formatWithOptions,
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
  isDate,
  isPrimitive,
  isBuffer,
  _extend,
  getSystemErrorName,
  deprecate,
  callbackify,
  promisify,
  inherits,
  types,
  stripVTControlCharacters,
  TextDecoder,
  TextEncoder,
  log,
  debuglog,
  isDeepStrictEqual,
};
