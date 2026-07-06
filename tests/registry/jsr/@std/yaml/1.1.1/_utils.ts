// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.

export function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object";
}

export function isNegativeZero(i: number): boolean {
  return i === 0 && Number.NEGATIVE_INFINITY === 1 / i;
}

export function isPlainObject(object: unknown): object is object {
  return Object.prototype.toString.call(object) === "[object Object]";
}
