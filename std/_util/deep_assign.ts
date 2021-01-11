// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert } from "./assert.ts";

export function deepAssign<T, U>(target: T, source: U): T & U;
export function deepAssign<T, U, V>(
  target: T,
  source1: U,
  source2: V,
): T & U & V;
export function deepAssign<T, U, V, W>(
  target: T,
  source1: U,
  source2: V,
  source3: W,
): T & U & V & W;
export function deepAssign(
  // deno-lint-ignore no-explicit-any
  target: Record<string, any>,
  // deno-lint-ignore no-explicit-any
  ...sources: any[]
): // deno-lint-ignore ban-types
object | undefined {
  for (let i = 0; i < sources.length; i++) {
    const source = sources[i];
    if (!source || typeof source !== `object`) {
      return;
    }
    Object.entries(source).forEach(([key, value]): void => {
      if (value instanceof Date) {
        target[key] = new Date(value);
        return;
      }
      if (!value || typeof value !== `object`) {
        target[key] = value;
        return;
      }
      if (Array.isArray(value)) {
        target[key] = [];
      }
      // value is an Object
      if (typeof target[key] !== `object` || !target[key]) {
        target[key] = {};
      }
      assert(value);
      deepAssign(target[key] as Record<string, unknown>, value);
    });
  }
  return target;
}
