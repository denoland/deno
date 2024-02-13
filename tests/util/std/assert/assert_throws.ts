// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertIsError } from "./assert_is_error.ts";
import { AssertionError } from "./assertion_error.ts";

/**
 * Executes a function, expecting it to throw. If it does not, then it
 * throws.
 *
 * @example
 * ```ts
 * import { assertThrows } from "https://deno.land/std@$STD_VERSION/assert/assert_throws.ts";
 *
 * Deno.test("doesThrow", function (): void {
 *   assertThrows((): void => {
 *     throw new TypeError("hello world!");
 *   });
 * });
 *
 * // This test will not pass.
 * Deno.test("fails", function (): void {
 *   assertThrows((): void => {
 *     console.log("Hello world");
 *   });
 * });
 * ```
 */
export function assertThrows(
  fn: () => unknown,
  msg?: string,
): unknown;
/**
 * Executes a function, expecting it to throw. If it does not, then it
 * throws. An error class and a string that should be included in the
 * error message can also be asserted.
 *
 * @example
 * ```ts
 * import { assertThrows } from "https://deno.land/std@$STD_VERSION/assert/assert_throws.ts";
 *
 * Deno.test("doesThrow", function (): void {
 *   assertThrows((): void => {
 *     throw new TypeError("hello world!");
 *   }, TypeError);
 *   assertThrows(
 *     (): void => {
 *       throw new TypeError("hello world!");
 *     },
 *     TypeError,
 *     "hello",
 *   );
 * });
 *
 * // This test will not pass.
 * Deno.test("fails", function (): void {
 *   assertThrows((): void => {
 *     console.log("Hello world");
 *   });
 * });
 * ```
 */
export function assertThrows<E extends Error = Error>(
  fn: () => unknown,
  // deno-lint-ignore no-explicit-any
  ErrorClass: new (...args: any[]) => E,
  msgIncludes?: string,
  msg?: string,
): E;
export function assertThrows<E extends Error = Error>(
  fn: () => unknown,
  errorClassOrMsg?:
    // deno-lint-ignore no-explicit-any
    | (new (...args: any[]) => E)
    | string,
  msgIncludesOrMsg?: string,
  msg?: string,
): E | Error | unknown {
  // deno-lint-ignore no-explicit-any
  let ErrorClass: (new (...args: any[]) => E) | undefined = undefined;
  let msgIncludes: string | undefined = undefined;
  let err;

  if (typeof errorClassOrMsg !== "string") {
    if (
      errorClassOrMsg === undefined ||
      errorClassOrMsg.prototype instanceof Error ||
      errorClassOrMsg.prototype === Error.prototype
    ) {
      // deno-lint-ignore no-explicit-any
      ErrorClass = errorClassOrMsg as new (...args: any[]) => E;
      msgIncludes = msgIncludesOrMsg;
    } else {
      msg = msgIncludesOrMsg;
    }
  } else {
    msg = errorClassOrMsg;
  }
  let doesThrow = false;
  const msgSuffix = msg ? `: ${msg}` : ".";
  try {
    fn();
  } catch (error) {
    if (ErrorClass) {
      if (error instanceof Error === false) {
        throw new AssertionError(`A non-Error object was thrown${msgSuffix}`);
      }
      assertIsError(
        error,
        ErrorClass,
        msgIncludes,
        msg,
      );
    }
    err = error;
    doesThrow = true;
  }
  if (!doesThrow) {
    msg = `Expected function to throw${msgSuffix}`;
    throw new AssertionError(msg);
  }
  return err;
}
