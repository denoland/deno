// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

export const isArray = Array.isArray

export function isBoolean(value: unknown): value is boolean {
  return typeof value === "boolean" || value instanceof Boolean;
}

export function isNull(value: unknown): value is null {
  return value === null;
}

export function isNullOrUndefined(value: unknown): value is null | undefined {
  return value === null || value === undefined;
}

export function isNumber(value: unknown): value is number {
  return typeof value === "number" || value instanceof Number;
}

export function isString(value: unknown): value is string {
  return typeof value === "string" || value instanceof String;
}

export function isSymbol(value: unknown): value is symbol {
  return typeof value === "symbol";
}

export function isUndefined(value: unknown): value is undefined {
  return value === undefined;
}

export function isObject(value: unknown): value is object {
  return value !== null && typeof value === "object";
}
  
export function isError(e: unknown): boolean {
  return e instanceof Error;
}

export function isFunction(value: unknown): value is Function {
  return typeof value === "function";
}

export function isRegExp(value: unknown): value is RegExp {
  return value instanceof RegExp;
}

export function isNegativeZero(i: number): boolean {
  return i === 0 && Number.NEGATIVE_INFINITY === 1 / i;
}

export function isNothing(value: unknown): value is never {
  return typeof value === "undefined" || value === null;
}

export function toArray<T>(sequence: T): T | [] | [T] {
  if (isArray(sequence)) return sequence;
  if (isNothing(sequence)) return [];
  return [sequence];
}