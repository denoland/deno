// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { AssertionError } from "./assertion_error.ts";
import { format } from "./_format.ts";

/**
 * Make an assertion that `actual` and `expected` are not strictly equal.
 * If the values are strictly equal then throw.
 *
 * ```ts
 * import { assertNotStrictEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_not_strict_equals.ts";
 *
 * assertNotStrictEquals(1, 1)
 * ```
 */
export function assertNotStrictEquals<T>(
  actual: T,
  expected: T,
  msg?: string,
) {
  if (!Object.is(actual, expected)) {
    return;
  }

  const msgSuffix = msg ? `: ${msg}` : ".";
  throw new AssertionError(
    `Expected "actual" to not be strictly equal to: ${
      format(actual)
    }${msgSuffix}\n`,
  );
}
