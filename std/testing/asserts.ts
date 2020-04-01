// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { red, green, white, gray, bold } from "../fmt/colors.ts";
import diff, { DiffType, DiffResult } from "./diff.ts";
import { format } from "./format.ts";

const CAN_NOT_DISPLAY = "[Cannot display]";

interface Constructor {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  new (...args: any[]): any;
}

export class AssertionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AssertionError";
  }
}

function createStr(v: unknown): string {
  try {
    return format(v);
  } catch (e) {
    return red(CAN_NOT_DISPLAY);
  }
}

function createColor(diffType: DiffType): (s: string) => string {
  switch (diffType) {
    case DiffType.added:
      return (s: string): string => green(bold(s));
    case DiffType.removed:
      return (s: string): string => red(bold(s));
    default:
      return white;
  }
}

function createSign(diffType: DiffType): string {
  switch (diffType) {
    case DiffType.added:
      return "+   ";
    case DiffType.removed:
      return "-   ";
    default:
      return "    ";
  }
}

function buildMessage(diffResult: ReadonlyArray<DiffResult<string>>): string[] {
  const messages: string[] = [];
  messages.push("");
  messages.push("");
  messages.push(
    `    ${gray(bold("[Diff]"))} ${red(bold("Left"))} / ${green(bold("Right"))}`
  );
  messages.push("");
  messages.push("");
  diffResult.forEach((result: DiffResult<string>): void => {
    const c = createColor(result.type);
    messages.push(c(`${createSign(result.type)}${result.value}`));
  });
  messages.push("");

  return messages;
}

function isKeyedCollection(x: unknown): x is Set<unknown> {
  return [Symbol.iterator, "size"].every((k) => k in (x as Set<unknown>));
}

export function equal(c: unknown, d: unknown): boolean {
  const seen = new Map();
  return (function compare(a: unknown, b: unknown): boolean {
    // Have to render RegExp & Date for string comparison
    // unless it's mistreated as object
    if (
      a &&
      b &&
      ((a instanceof RegExp && b instanceof RegExp) ||
        (a instanceof Date && b instanceof Date))
    ) {
      return String(a) === String(b);
    }
    if (Object.is(a, b)) {
      return true;
    }
    if (a && typeof a === "object" && b && typeof b === "object") {
      if (seen.get(a) === b) {
        return true;
      }
      if (Object.keys(a || {}).length !== Object.keys(b || {}).length) {
        return false;
      }
      if (isKeyedCollection(a) && isKeyedCollection(b)) {
        if (a.size !== b.size) {
          return false;
        }

        let unmatchedEntries = a.size;

        for (const [aKey, aValue] of a.entries()) {
          for (const [bKey, bValue] of b.entries()) {
            /* Given that Map keys can be references, we need
             * to ensure that they are also deeply equal */
            if (
              (aKey === aValue && bKey === bValue && compare(aKey, bKey)) ||
              (compare(aKey, bKey) && compare(aValue, bValue))
            ) {
              unmatchedEntries--;
            }
          }
        }

        return unmatchedEntries === 0;
      }
      const merged = { ...a, ...b };
      for (const key in merged) {
        type Key = keyof typeof merged;
        if (!compare(a && a[key as Key], b && b[key as Key])) {
          return false;
        }
      }
      seen.set(a, b);
      return true;
    }
    return false;
  })(c, d);
}

/** Make an assertion, if not `true`, then throw. */
export function assert(expr: unknown, msg = ""): asserts expr {
  if (!expr) {
    throw new AssertionError(msg);
  }
}

/**
 * Make an assertion that `actual` and `expected` are equal, deeply. If not
 * deeply equal, then throw.
 */
export function assertEquals(
  actual: unknown,
  expected: unknown,
  msg?: string
): void {
  if (equal(actual, expected)) {
    return;
  }
  let message = "";
  const actualString = createStr(actual);
  const expectedString = createStr(expected);
  try {
    const diffResult = diff(
      actualString.split("\n"),
      expectedString.split("\n")
    );
    message = buildMessage(diffResult).join("\n");
  } catch (e) {
    message = `\n${red(CAN_NOT_DISPLAY)} + \n\n`;
  }
  if (msg) {
    message = msg;
  }
  throw new AssertionError(message);
}

/**
 * Make an assertion that `actual` and `expected` are not equal, deeply.
 * If not then throw.
 */
export function assertNotEquals(
  actual: unknown,
  expected: unknown,
  msg?: string
): void {
  if (!equal(actual, expected)) {
    return;
  }
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
  if (!msg) {
    msg = `actual: ${actualString} expected: ${expectedString}`;
  }
  throw new AssertionError(msg);
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
    if (!msg) {
      msg = `actual: ${actualString} expected: ${expectedString}`;
    }
    throw new AssertionError(msg);
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
    if (!msg) {
      msg = `actual: "${actual}" expected to contains: "${expected}"`;
    }
    throw new AssertionError(msg);
  }
}

/**
 * Make an assertion that `actual` contains the `expected` values
 * If not then thrown.
 */
export function assertArrayContains(
  actual: unknown[],
  expected: unknown[],
  msg?: string
): void {
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
  if (!msg) {
    msg = `actual: "${actual}" expected to contains: "${expected}"`;
    msg += "\n";
    msg += `missing: ${missing}`;
  }
  throw new AssertionError(msg);
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
    if (!msg) {
      msg = `actual: "${actual}" expected to match: "${expected}"`;
    }
    throw new AssertionError(msg);
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
): Error {
  let doesThrow = false;
  let error = null;
  try {
    fn();
  } catch (e) {
    if (ErrorClass && !(Object.getPrototypeOf(e) === ErrorClass.prototype)) {
      msg = `Expected error to be instance of "${ErrorClass.name}", but was "${
        e.constructor.name
      }"${msg ? `: ${msg}` : "."}`;
      throw new AssertionError(msg);
    }
    if (msgIncludes && !e.message.includes(msgIncludes)) {
      msg = `Expected error message to include "${msgIncludes}", but got "${
        e.message
      }"${msg ? `: ${msg}` : "."}`;
      throw new AssertionError(msg);
    }
    doesThrow = true;
    error = e;
  }
  if (!doesThrow) {
    msg = `Expected function to throw${msg ? `: ${msg}` : "."}`;
    throw new AssertionError(msg);
  }
  return error;
}

export async function assertThrowsAsync(
  fn: () => Promise<void>,
  ErrorClass?: Constructor,
  msgIncludes = "",
  msg?: string
): Promise<Error> {
  let doesThrow = false;
  let error = null;
  try {
    await fn();
  } catch (e) {
    if (ErrorClass && !(Object.getPrototypeOf(e) === ErrorClass.prototype)) {
      msg = `Expected error to be instance of "${ErrorClass.name}", but got "${
        e.name
      }"${msg ? `: ${msg}` : "."}`;
      throw new AssertionError(msg);
    }
    if (msgIncludes && !e.message.includes(msgIncludes)) {
      msg = `Expected error message to include "${msgIncludes}", but got "${
        e.message
      }"${msg ? `: ${msg}` : "."}`;
      throw new AssertionError(msg);
    }
    doesThrow = true;
    error = e;
  }
  if (!doesThrow) {
    msg = `Expected function to throw${msg ? `: ${msg}` : "."}`;
    throw new AssertionError(msg);
  }
  return error;
}

/** Use this to stub out methods that will throw when invoked. */
export function unimplemented(msg?: string): never {
  throw new AssertionError(msg || "unimplemented");
}

/** Use this to assert unreachable code. */
export function unreachable(): never {
  throw new AssertionError("unreachable");
}

export function assertNotEOF<T extends {}>(val: T | Deno.EOF): T {
  assertNotEquals(val, Deno.EOF);
  return val as T;
}
