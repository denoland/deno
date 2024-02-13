// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { AssertionError } from "./assertion_error.ts";
import { assertIsError } from "./assert_is_error.ts";

/**
 * Executes a function which returns a promise, expecting it to reject.
 *
 * @example
 * ```ts
 * import { assertRejects } from "https://deno.land/std@$STD_VERSION/assert/assert_rejects.ts";
 *
 * Deno.test("doesThrow", async function () {
 *   await assertRejects(
 *     async () => {
 *       throw new TypeError("hello world!");
 *     },
 *   );
 *   await assertRejects(
 *     async () => {
 *       return Promise.reject(new Error());
 *     },
 *   );
 * });
 *
 * // This test will not pass.
 * Deno.test("fails", async function () {
 *   await assertRejects(
 *     async () => {
 *       console.log("Hello world");
 *     },
 *   );
 * });
 * ```
 */
export function assertRejects(
  fn: () => PromiseLike<unknown>,
  msg?: string,
): Promise<unknown>;
/**
 * Executes a function which returns a promise, expecting it to reject.
 * If it does not, then it throws. An error class and a string that should be
 * included in the error message can also be asserted.
 *
 * @example
 * ```ts
 * import { assertRejects } from "https://deno.land/std@$STD_VERSION/assert/assert_rejects.ts";
 *
 * Deno.test("doesThrow", async function () {
 *   await assertRejects(async () => {
 *     throw new TypeError("hello world!");
 *   }, TypeError);
 *   await assertRejects(
 *     async () => {
 *       throw new TypeError("hello world!");
 *     },
 *     TypeError,
 *     "hello",
 *   );
 * });
 *
 * // This test will not pass.
 * Deno.test("fails", async function () {
 *   await assertRejects(
 *     async () => {
 *       console.log("Hello world");
 *     },
 *   );
 * });
 * ```
 */
export function assertRejects<E extends Error = Error>(
  fn: () => PromiseLike<unknown>,
  // deno-lint-ignore no-explicit-any
  ErrorClass: new (...args: any[]) => E,
  msgIncludes?: string,
  msg?: string,
): Promise<E>;
export async function assertRejects<E extends Error = Error>(
  fn: () => PromiseLike<unknown>,
  errorClassOrMsg?:
    // deno-lint-ignore no-explicit-any
    | (new (...args: any[]) => E)
    | string,
  msgIncludesOrMsg?: string,
  msg?: string,
): Promise<E | Error | unknown> {
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
    }
  } else {
    msg = errorClassOrMsg;
  }
  let doesThrow = false;
  let isPromiseReturned = false;
  const msgSuffix = msg ? `: ${msg}` : ".";
  try {
    const possiblePromise = fn();
    if (
      possiblePromise &&
      typeof possiblePromise === "object" &&
      typeof possiblePromise.then === "function"
    ) {
      isPromiseReturned = true;
      await possiblePromise;
    }
  } catch (error) {
    if (!isPromiseReturned) {
      throw new AssertionError(
        `Function throws when expected to reject${msgSuffix}`,
      );
    }
    if (ErrorClass) {
      if (error instanceof Error === false) {
        throw new AssertionError(`A non-Error object was rejected${msgSuffix}`);
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
    throw new AssertionError(
      `Expected function to reject${msgSuffix}`,
    );
  }
  return err;
}
