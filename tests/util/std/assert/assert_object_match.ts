// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./assert_equals.ts";

/**
 * Make an assertion that `actual` object is a subset of `expected` object, deeply.
 * If not, then throw.
 */
export function assertObjectMatch(
  // deno-lint-ignore no-explicit-any
  actual: Record<PropertyKey, any>,
  expected: Record<PropertyKey, unknown>,
  msg?: string,
) {
  type loose = Record<PropertyKey, unknown>;

  function filter(a: loose, b: loose) {
    const seen = new WeakMap();
    return fn(a, b);

    function fn(a: loose, b: loose): loose {
      // Prevent infinite loop with circular references with same filter
      if ((seen.has(a)) && (seen.get(a) === b)) {
        return a;
      }
      try {
        seen.set(a, b);
      } catch (err) {
        if (err instanceof TypeError) {
          throw new TypeError(
            `Cannot assertObjectMatch ${
              a === null ? null : `type ${typeof a}`
            }`,
          );
        } else throw err;
      }
      // Filter keys and symbols which are present in both actual and expected
      const filtered = {} as loose;
      const entries = [
        ...Object.getOwnPropertyNames(a),
        ...Object.getOwnPropertySymbols(a),
      ]
        .filter((key) => key in b)
        .map((key) => [key, a[key as string]]) as Array<[string, unknown]>;
      for (const [key, value] of entries) {
        // On array references, build a filtered array and filter nested objects inside
        if (Array.isArray(value)) {
          const subset = (b as loose)[key];
          if (Array.isArray(subset)) {
            filtered[key] = fn({ ...value }, { ...subset });
            continue;
          }
        } // On regexp references, keep value as it to avoid loosing pattern and flags
        else if (value instanceof RegExp) {
          filtered[key] = value;
          continue;
        } // On nested objects references, build a filtered object recursively
        else if (typeof value === "object" && value !== null) {
          const subset = (b as loose)[key];
          if ((typeof subset === "object") && subset) {
            // When both operands are maps, build a filtered map with common keys and filter nested objects inside
            if ((value instanceof Map) && (subset instanceof Map)) {
              filtered[key] = new Map(
                [...value].filter(([k]) => subset.has(k)).map((
                  [k, v],
                ) => [k, typeof v === "object" ? fn(v, subset.get(k)) : v]),
              );
              continue;
            }
            // When both operands are set, build a filtered set with common values
            if ((value instanceof Set) && (subset instanceof Set)) {
              filtered[key] = new Set([...value].filter((v) => subset.has(v)));
              continue;
            }
            filtered[key] = fn(value as loose, subset as loose);
            continue;
          }
        }
        filtered[key] = value;
      }
      return filtered;
    }
  }
  return assertEquals(
    // get the intersection of "actual" and "expected"
    // side effect: all the instances' constructor field is "Object" now.
    filter(actual, expected),
    // set (nested) instances' constructor field to be "Object" without changing expected value.
    // see https://github.com/denoland/deno_std/pull/1419
    filter(expected, expected),
    msg,
  );
}
