// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { AssertionError } from "./assertion_error.ts";

/**
 * Make an assertion that actual includes expected. If not
 * then throw.
 */
export function assertStringIncludes(
  actual: string,
  expected: string,
  msg?: string,
) {
  if (!actual.includes(expected)) {
    const msgSuffix = msg ? `: ${msg}` : ".";
    msg = `Expected actual: "${actual}" to contain: "${expected}"${msgSuffix}`;
    throw new AssertionError(msg);
  }
}
