// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { AssertionError } from "./assertion_error.ts";

/**
 * Make an assertion that `actual` match RegExp `expected`. If not
 * then throw.
 */
export function assertMatch(
  actual: string,
  expected: RegExp,
  msg?: string,
) {
  if (!expected.test(actual)) {
    const msgSuffix = msg ? `: ${msg}` : ".";
    msg = `Expected actual: "${actual}" to match: "${expected}"${msgSuffix}`;
    throw new AssertionError(msg);
  }
}
