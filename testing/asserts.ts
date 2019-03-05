// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assertEqual as prettyAssertEqual } from "./pretty.ts";

interface Constructor {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  new (...args: any[]): any;
}

/** Make an assertion, if not `true`, then throw. */
export function assert(expr: boolean, msg = ""): void {
  if (!expr) {
    throw new Error(msg);
  }
}

/**
 * Make an assertion that `actual` and `expected` are equal, deeply. If not
 * deeply equal, then throw.
 */
export function equal(actual: unknown, expected: unknown, msg?: string): void {
  prettyAssertEqual(actual, expected, msg);
}

/**
 * Make an assertion that `actual` and `expected` are strictly equal.  If
 * not then throw.
 */
export function assertStrictEq(
  actual: unknown,
  expected: unknown,
  msg?: string
): void {
  if (actual !== expected) {
    let actualString: string;
    let expectedString: string;
    try {
      actualString = String(actual);
    } catch (e) {
      actualString = "[Cannot display]";
    }
    try {
      expectedString = String(expected);
    } catch (e) {
      expectedString = "[Cannot display]";
    }
    console.error(
      "strictEqual failed. actual =",
      actualString,
      "expected =",
      expectedString
    );
    if (!msg) {
      msg = `actual: ${actualString} expected: ${expectedString}`;
    }
    throw new Error(msg);
  }
}

/**
 * Make an assertion that actual contains expected. If not
 * then thrown.
 */
export function assertStrContains(
  actual: string,
  expected: string,
  msg?: string
): void {
  if (!actual.includes(expected)) {
    console.error(
      "stringContains failed. actual =",
      actual,
      "not containing ",
      expected
    );
    if (!msg) {
      msg = `actual: "${actual}" expected to contains: "${expected}"`;
    }
    throw new Error(msg);
  }
}

/**
 * Make an assertion that `actual` match RegExp `expected`. If not
 * then thrown
 */
export function assertMatch(
  actual: string,
  expected: RegExp,
  msg?: string
): void {
  if (!expected.test(actual)) {
    console.error(
      "stringMatching failed. actual =",
      actual,
      "not matching RegExp ",
      expected
    );
    if (!msg) {
      msg = `actual: "${actual}" expected to match: "${expected}"`;
    }
    throw new Error(msg);
  }
}

/**
 * Forcefully throws a failed assertion
 */
export function fail(msg?: string): void {
  // eslint-disable-next-line @typescript-eslint/no-use-before-define
  assert(false, `Failed assertion${msg ? `: ${msg}` : "."}`);
}

/** Executes a function, expecting it to throw.  If it does not, then it
 * throws.  An error class and a string that should be included in the
 * error message can also be asserted.
 */
export function assertThrows(
  fn: () => void,
  ErrorClass?: Constructor,
  msgIncludes = "",
  msg?: string
): void {
  let doesThrow = false;
  try {
    fn();
  } catch (e) {
    if (ErrorClass && !(Object.getPrototypeOf(e) === ErrorClass.prototype)) {
      msg = `Expected error to be instance of "${ErrorClass.name}"${
        msg ? `: ${msg}` : "."
      }`;
      throw new Error(msg);
    }
    if (msgIncludes) {
      if (!e.message.includes(msgIncludes)) {
        msg = `Expected error message to include "${msgIncludes}", but got "${
          e.message
        }"${msg ? `: ${msg}` : "."}`;
        throw new Error(msg);
      }
    }
    doesThrow = true;
  }
  if (!doesThrow) {
    msg = `Expected function to throw${msg ? `: ${msg}` : "."}`;
    throw new Error(msg);
  }
}

export async function assertThrowsAsync(
  fn: () => Promise<void>,
  ErrorClass?: Constructor,
  msgIncludes = "",
  msg?: string
): Promise<void> {
  let doesThrow = false;
  try {
    await fn();
  } catch (e) {
    if (ErrorClass && !(Object.getPrototypeOf(e) === ErrorClass.prototype)) {
      msg = `Expected error to be instance of "${ErrorClass.name}"${
        msg ? `: ${msg}` : "."
      }`;
      throw new Error(msg);
    }
    if (msgIncludes) {
      if (!e.message.includes(msgIncludes)) {
        msg = `Expected error message to include "${msgIncludes}", but got "${
          e.message
        }"${msg ? `: ${msg}` : "."}`;
        throw new Error(msg);
      }
    }
    doesThrow = true;
  }
  if (!doesThrow) {
    msg = `Expected function to throw${msg ? `: ${msg}` : "."}`;
    throw new Error(msg);
  }
}
