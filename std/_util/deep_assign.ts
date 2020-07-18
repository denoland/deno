// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../_util/assert.ts";

export function deepAssign(
  target: Record<string, unknown>,
  ...sources: object[]
): object | undefined {
  for (let i = 0; i < sources.length; i++) {
    const source = sources[i];
    if (!source || typeof source !== `object`) {
      return;
    }
    Object.entries(source).forEach(([key, value]: [string, unknown]): void => {
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
