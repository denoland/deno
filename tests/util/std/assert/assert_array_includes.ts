// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { equal } from "./equal.ts";
import { format } from "./_format.ts";
import { AssertionError } from "./assertion_error.ts";

/**
 * Make an assertion that `actual` includes the `expected` values.
 * If not then an error will be thrown.
 *
 * Type parameter can be specified to ensure values under comparison have the same type.
 *
 * @example
 * ```ts
 * import { assertArrayIncludes } from "https://deno.land/std@$STD_VERSION/assert/assert_array_includes.ts";
 *
 * assertArrayIncludes<number>([1, 2], [2])
 * ```
 */
export function assertArrayIncludes<T>(
  actual: ArrayLike<T>,
  expected: ArrayLike<T>,
  msg?: string,
) {
  const missing: unknown[] = [];
  for (let i = 0; i < expected.length; i++) {
    let found = false;
    for (let j = 0; j < actual.length; j++) {
      if (equal(expected[i], actual[j])) {
        found = true;
        break;
      }
    }
    if (!found) {
      missing.push(expected[i]);
    }
  }
  if (missing.length === 0) {
    return;
  }

  const msgSuffix = msg ? `: ${msg}` : ".";
  msg = `Expected actual: "${format(actual)}" to include: "${
    format(expected)
  }"${msgSuffix}\nmissing: ${format(missing)}`;
  throw new AssertionError(msg);
}
