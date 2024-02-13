// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { AssertionError } from "./assertion_error.ts";
import { stripColor } from "../fmt/colors.ts";

/**
 * Make an assertion that `error` is an `Error`.
 * If not then an error will be thrown.
 * An error class and a string that should be included in the
 * error message can also be asserted.
 */
export function assertIsError<E extends Error = Error>(
  error: unknown,
  // deno-lint-ignore no-explicit-any
  ErrorClass?: new (...args: any[]) => E,
  msgIncludes?: string,
  msg?: string,
): asserts error is E {
  const msgSuffix = msg ? `: ${msg}` : ".";
  if (error instanceof Error === false) {
    throw new AssertionError(
      `Expected "error" to be an Error object${msgSuffix}}`,
    );
  }
  if (ErrorClass && !(error instanceof ErrorClass)) {
    msg = `Expected error to be instance of "${ErrorClass.name}", but was "${
      typeof error === "object" ? error?.constructor?.name : "[not an object]"
    }"${msgSuffix}`;
    throw new AssertionError(msg);
  }
  if (
    msgIncludes && (!(error instanceof Error) ||
      !stripColor(error.message).includes(stripColor(msgIncludes)))
  ) {
    msg = `Expected error message to include ${
      JSON.stringify(msgIncludes)
    }, but got ${
      error instanceof Error
        ? JSON.stringify(error.message)
        : '"[not an Error]"' // TODO(kt3k): show more useful information
    }${msgSuffix}`;
    throw new AssertionError(msg);
  }
}
