export { promisify } from "./_util/_util_promisify.ts";
export { callbackify } from "./_util/_util_callbackify.ts";
import * as types from "./_util/_util_types.ts";

export { types };

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function inspect(object: unknown, ...opts: any): string {
  return Deno.inspect(object, {
    depth: opts.depth ?? 4,
    iterableLimit: opts.iterableLimit ?? 100,
    compact: !!(opts.compact ?? true),
    sorted: !!(opts.sorted ?? false),
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

export function validateIntegerRange(
  value: number,
  name: string,
  min = -2147483648,
  max = 2147483647,
): void {
  // The defaults for min and max correspond to the limits of 32-bit integers.
  if (!Number.isInteger(value)) {
    throw new Error(`${name} must be 'an integer' but was ${value}`);
  }
  if (value < min || value > max) {
    throw new Error(
      `${name} must be >= ${min} && <= ${max}.  Value was ${value}`,
    );
  }
}

import { _TextDecoder, _TextEncoder } from "./_utils.ts";

/** The global TextDecoder */
export type TextDecoder = import("./_utils.ts")._TextDecoder;
export const TextDecoder = _TextDecoder;

/** The global TextEncoder */
export type TextEncoder = import("./_utils.ts")._TextEncoder;
export const TextEncoder = _TextEncoder;
