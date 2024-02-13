// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertNotStrictEquals } from "../assert/assert_not_strict_equals.ts";
import { assertStrictEquals } from "../assert/assert_strict_equals.ts";
import { assertInstanceOf } from "../assert/assert_instance_of.ts";
import { assertIsError } from "../assert/assert_is_error.ts";
import { assertNotInstanceOf } from "../assert/assert_not_instance_of.ts";
import { assertNotEquals } from "../assert/assert_not_equals.ts";
import { assertEquals } from "../assert/assert_equals.ts";
import { assertMatch } from "../assert/assert_match.ts";
import { assertObjectMatch } from "../assert/assert_object_match.ts";
import { assertNotMatch } from "../assert/assert_not_match.ts";
import { AssertionError } from "../assert/assertion_error.ts";
import { equal } from "../assert/equal.ts";
import { format } from "../assert/_format.ts";

import { AnyConstructor, MatcherContext, MatchResult } from "./_types.ts";
import { getMockCalls } from "./_mock_util.ts";
import { inspectArg, inspectArgs } from "./_inspect_args.ts";

export function toBe(context: MatcherContext, expect: unknown): MatchResult {
  if (context.isNot) {
    assertNotStrictEquals(context.value, expect, context.customMessage);
  } else {
    assertStrictEquals(context.value, expect, context.customMessage);
  }
}

export function toEqual(
  context: MatcherContext,
  expected: unknown,
): MatchResult {
  if (context.isNot) {
    assertNotEquals(context.value, expected, context.customMessage);
  } else {
    assertEquals(context.value, expected, context.customMessage);
  }
}

export function toStrictEqual(
  context: MatcherContext,
  expected: unknown,
): MatchResult {
  if (context.isNot) {
    assertNotStrictEquals(
      context.value,
      expected,
      context.customMessage,
    );
  } else {
    assertStrictEquals(context.value, expected, context.customMessage);
  }
}

export function toBeCloseTo(
  context: MatcherContext,
  expected: number,
  numDigits = 2,
): MatchResult {
  if (numDigits < 0) {
    throw new Error(
      "toBeCloseTo second argument must be a non-negative integer. Got " +
        numDigits,
    );
  }
  const tolerance = 0.5 * Math.pow(10, -numDigits);
  const value = Number(context.value);
  const pass = Math.abs(expected - value) < tolerance;

  if (context.isNot) {
    if (pass) {
      throw new AssertionError(
        `Expected the value not to be close to ${expected} (using ${numDigits} digits), but it is`,
      );
    }
  } else {
    if (!pass) {
      throw new AssertionError(
        `Expected the value (${value} to be close to ${expected} (using ${numDigits} digits), but it is not`,
      );
    }
  }
}

export function toBeDefined(context: MatcherContext): MatchResult {
  if (context.isNot) {
    assertStrictEquals(context.value, undefined, context.customMessage);
  } else {
    assertNotStrictEquals(context.value, undefined, context.customMessage);
  }
}

export function toBeUndefined(context: MatcherContext): MatchResult {
  if (context.isNot) {
    assertNotStrictEquals(
      context.value,
      undefined,
      context.customMessage,
    );
  } else {
    assertStrictEquals(context.value, undefined, context.customMessage);
  }
}

export function toBeFalsy(
  context: MatcherContext,
): MatchResult {
  const isFalsy = !(context.value);
  if (context.isNot) {
    if (isFalsy) {
      throw new AssertionError(
        `Expected ${context.value} to NOT be falsy`,
      );
    }
  } else {
    if (!isFalsy) {
      throw new AssertionError(
        `Expected ${context.value} to be falsy`,
      );
    }
  }
}

export function toBeTruthy(
  context: MatcherContext,
): MatchResult {
  const isTruthy = !!(context.value);
  if (context.isNot) {
    if (isTruthy) {
      throw new AssertionError(
        `Expected ${context.value} to NOT be truthy`,
      );
    }
  } else {
    if (!isTruthy) {
      throw new AssertionError(
        `Expected ${context.value} to be truthy`,
      );
    }
  }
}

export function toBeGreaterThanOrEqual(
  context: MatcherContext,
  expected: number,
): MatchResult {
  const isGreaterOrEqual = Number(context.value) >= Number(expected);
  if (context.isNot) {
    if (isGreaterOrEqual) {
      throw new AssertionError(
        `Expected ${context.value} to NOT be greater than or equal ${expected}`,
      );
    }
  } else {
    if (!isGreaterOrEqual) {
      throw new AssertionError(
        `Expected ${context.value} to be greater than or equal ${expected}`,
      );
    }
  }
}

export function toBeGreaterThan(
  context: MatcherContext,
  expected: number,
): MatchResult {
  const isGreater = Number(context.value) > Number(expected);
  if (context.isNot) {
    if (isGreater) {
      throw new AssertionError(
        `Expected ${context.value} to NOT be greater than ${expected}`,
      );
    }
  } else {
    if (!isGreater) {
      throw new AssertionError(
        `Expected ${context.value} to be greater than ${expected}`,
      );
    }
  }
}

export function toBeInstanceOf<T extends AnyConstructor>(
  context: MatcherContext,
  expected: T,
): MatchResult {
  if (context.isNot) {
    assertNotInstanceOf(context.value, expected);
  } else {
    assertInstanceOf(context.value, expected);
  }
}
export function toBeLessThanOrEqual(
  context: MatcherContext,
  expected: number,
): MatchResult {
  const isLower = Number(context.value) <= Number(expected);
  if (context.isNot) {
    if (isLower) {
      throw new AssertionError(
        `Expected ${context.value} to NOT be lower than or equal ${expected}`,
      );
    }
  } else {
    if (!isLower) {
      throw new AssertionError(
        `Expected ${context.value} to be lower than or equal ${expected}`,
      );
    }
  }
}
export function toBeLessThan(
  context: MatcherContext,
  expected: number,
): MatchResult {
  const isLower = Number(context.value) < Number(expected);
  if (context.isNot) {
    if (isLower) {
      throw new AssertionError(
        `Expected ${context.value} to NOT be lower than ${expected}`,
      );
    }
  } else {
    if (!isLower) {
      throw new AssertionError(
        `Expected ${context.value} to be lower than ${expected}`,
      );
    }
  }
}
export function toBeNaN(context: MatcherContext): MatchResult {
  if (context.isNot) {
    assertNotEquals(
      isNaN(Number(context.value)),
      true,
      context.customMessage || `Expected ${context.value} to not be NaN`,
    );
  } else {
    assertEquals(
      isNaN(Number(context.value)),
      true,
      context.customMessage || `Expected ${context.value} to be NaN`,
    );
  }
}

export function toBeNull(context: MatcherContext): MatchResult {
  if (context.isNot) {
    assertNotStrictEquals(
      context.value as number,
      null,
      context.customMessage || `Expected ${context.value} to not be null`,
    );
  } else {
    assertStrictEquals(
      context.value as number,
      null,
      context.customMessage || `Expected ${context.value} to be null`,
    );
  }
}

export function toHaveLength(
  context: MatcherContext,
  expected: number,
): MatchResult {
  const { value } = context;
  // deno-lint-ignore no-explicit-any
  const maybeLength = (value as any)?.length;
  const hasLength = maybeLength === expected;

  if (context.isNot) {
    if (hasLength) {
      throw new AssertionError(
        `Expected value not to have length ${expected}, but it does`,
      );
    }
  } else {
    if (!hasLength) {
      throw new AssertionError(
        `Expected value to have length ${expected}, but it does not. (The value has length ${maybeLength})`,
      );
    }
  }
}

export function toHaveProperty(
  context: MatcherContext,
  propName: string | string[],
  v?: unknown,
): MatchResult {
  const { value } = context;

  let propPath = [] as string[];
  if (Array.isArray(propName)) {
    propPath = propName;
  } else {
    propPath = propName.split(".");
  }

  // deno-lint-ignore no-explicit-any
  let current = value as any;
  while (true) {
    if (current === undefined || current === null) {
      break;
    }
    if (propPath.length === 0) {
      break;
    }
    const prop = propPath.shift()!;
    current = current[prop];
  }

  let hasProperty;
  if (v) {
    hasProperty = current !== undefined && propPath.length === 0 &&
      equal(current, v);
  } else {
    hasProperty = current !== undefined && propPath.length === 0;
  }

  let ofValue = "";
  if (v) {
    ofValue = ` of the value ${inspectArg(v)}`;
  }

  if (context.isNot) {
    if (hasProperty) {
      throw new AssertionError(
        `Expected the value not to have the property ${
          propPath.join(".")
        }${ofValue}, but it does.`,
      );
    }
  } else {
    if (!hasProperty) {
      throw new AssertionError(
        `Expected the value to have the property ${
          propPath.join(".")
        }${ofValue}, but it does not.`,
      );
    }
  }
}

export function toContain(
  context: MatcherContext,
  expected: unknown,
): MatchResult {
  // deno-lint-ignore no-explicit-any
  const doesContain = (context.value as any)?.includes?.(expected);

  if (context.isNot) {
    if (doesContain) {
      throw new AssertionError("The value contains the expected item");
    }
  } else {
    if (!doesContain) {
      throw new AssertionError("The value doesn't contain the expected item");
    }
  }
}

export function toContainEqual(
  context: MatcherContext,
  expected: unknown,
): MatchResult {
  const { value } = context;
  assertIsIterable(value);
  let doesContain = false;
  for (const item of value) {
    if (equal(item, expected)) {
      doesContain = true;
      break;
    }
  }

  if (context.isNot) {
    if (doesContain) {
      throw new AssertionError("The value contains the expected item");
    }
  } else {
    if (!doesContain) {
      throw new AssertionError("The value doesn't contain the expected item");
    }
  }
}

// deno-lint-ignore no-explicit-any
function assertIsIterable(value: any): asserts value is Iterable<unknown> {
  if (value == null) {
    throw new AssertionError("The value is null or undefined");
  }
  if (typeof value[Symbol.iterator] !== "function") {
    throw new AssertionError("The value is not iterable");
  }
}

export function toMatch(
  context: MatcherContext,
  expected: RegExp,
): MatchResult {
  if (context.isNot) {
    assertNotMatch(
      String(context.value),
      expected,
      context.customMessage,
    );
  } else {
    assertMatch(String(context.value), expected, context.customMessage);
  }
}

export function toMatchObject(
  context: MatcherContext,
  expected: Record<PropertyKey, unknown>,
): MatchResult {
  if (context.isNot) {
    let objectMatch = false;
    try {
      assertObjectMatch(
        // deno-lint-ignore no-explicit-any
        context.value as Record<PropertyKey, any>,
        expected,
        context.customMessage,
      );
      objectMatch = true;
      const actualString = format(context.value);
      const expectedString = format(expected);
      throw new AssertionError(
        `Expected ${actualString} to NOT match ${expectedString}`,
      );
    } catch (e) {
      if (objectMatch) {
        throw e;
      }
      return;
    }
  } else {
    assertObjectMatch(
      // deno-lint-ignore no-explicit-any
      context.value as Record<PropertyKey, any>,
      expected,
      context.customMessage,
    );
  }
}

export function toHaveBeenCalled(context: MatcherContext): MatchResult {
  const calls = getMockCalls(context.value);
  const hasBeenCalled = calls.length > 0;

  if (context.isNot) {
    if (hasBeenCalled) {
      throw new AssertionError(
        `Expected mock function not to be called, but it was called ${calls.length} time(s)`,
      );
    }
  } else {
    if (!hasBeenCalled) {
      throw new AssertionError(
        `Expected mock function to be called, but it was not called`,
      );
    }
  }
}

export function toHaveBeenCalledTimes(
  context: MatcherContext,
  expected: number,
): MatchResult {
  const calls = getMockCalls(context.value);

  if (context.isNot) {
    if (calls.length === expected) {
      throw new AssertionError(
        `Expected mock function not to be called ${expected} time(s), but it was`,
      );
    }
  } else {
    if (calls.length !== expected) {
      throw new AssertionError(
        `Expected mock function to be called ${expected} time(s), but it was called ${calls.length} time(s)`,
      );
    }
  }
}

export function toHaveBeenCalledWith(
  context: MatcherContext,
  ...expected: unknown[]
): MatchResult {
  const calls = getMockCalls(context.value);
  const hasBeenCalled = calls.some((call) => equal(call.args, expected));

  if (context.isNot) {
    if (hasBeenCalled) {
      throw new AssertionError(
        `Expected mock function not to be called with ${
          inspectArgs(expected)
        }, but it was`,
      );
    }
  } else {
    if (!hasBeenCalled) {
      let otherCalls = "";
      if (calls.length > 0) {
        otherCalls = `\n  Other calls:\n     ${
          calls.map((call) => inspectArgs(call.args)).join("\n    ")
        }`;
      }
      throw new AssertionError(
        `Expected mock function to be called with ${
          inspectArgs(expected)
        }, but it was not.${otherCalls}`,
      );
    }
  }
}
export function toHaveBeenLastCalledWith(
  context: MatcherContext,
  ...expected: unknown[]
): MatchResult {
  const calls = getMockCalls(context.value);
  const hasBeenCalled = calls.length > 0 &&
    equal(calls[calls.length - 1].args, expected);

  if (context.isNot) {
    if (hasBeenCalled) {
      throw new AssertionError(
        `Expected mock function not to be last called with ${
          inspectArgs(expected)
        }, but it was`,
      );
    }
  } else {
    if (!hasBeenCalled) {
      const lastCall = calls.at(-1);
      if (!lastCall) {
        throw new AssertionError(
          `Expected mock function to be last called with ${
            inspectArgs(expected)
          }, but it was not.`,
        );
      } else {
        throw new AssertionError(
          `Expected mock function to be last called with ${
            inspectArgs(expected)
          }, but it was last called with ${inspectArgs(lastCall.args)}.`,
        );
      }
    }
  }
}
export function toHaveBeenNthCalledWith(
  context: MatcherContext,
  nth: number,
  ...expected: unknown[]
): MatchResult {
  if (nth < 1) {
    new Error(`nth must be greater than 0. ${nth} was given.`);
  }

  const calls = getMockCalls(context.value);
  const callIndex = nth - 1;
  const hasBeenCalled = calls.length > callIndex &&
    equal(calls[callIndex].args, expected);

  if (context.isNot) {
    if (hasBeenCalled) {
      throw new AssertionError(
        `Expected the n-th call (n=${nth}) of mock function is not with ${
          inspectArgs(expected)
        }, but it was`,
      );
    }
  } else {
    if (!hasBeenCalled) {
      const nthCall = calls[callIndex];
      if (!nth) {
        throw new AssertionError(
          `Expected the n-th call (n=${nth}) of mock function is with ${
            inspectArgs(expected)
          }, but the n-th call does not exist.`,
        );
      } else {
        throw new AssertionError(
          `Expected the n-th call (n=${nth}) of mock function is with ${
            inspectArgs(expected)
          }, but it was with ${inspectArgs(nthCall.args)}.`,
        );
      }
    }
  }
}

export function toHaveReturned(context: MatcherContext): MatchResult {
  const calls = getMockCalls(context.value);
  const returned = calls.filter((call) => call.returns);

  if (context.isNot) {
    if (returned.length > 0) {
      throw new AssertionError(
        `Expected the mock function to not have returned, but it returned ${returned.length} times`,
      );
    }
  } else {
    if (returned.length === 0) {
      throw new AssertionError(
        `Expected the mock function to have returned, but it did not return`,
      );
    }
  }
}

export function toHaveReturnedTimes(
  context: MatcherContext,
  expected: number,
): MatchResult {
  const calls = getMockCalls(context.value);
  const returned = calls.filter((call) => call.returns);

  if (context.isNot) {
    if (returned.length === expected) {
      throw new AssertionError(
        `Expected the mock function to not have returned ${expected} times, but it returned ${returned.length} times`,
      );
    }
  } else {
    if (returned.length !== expected) {
      throw new AssertionError(
        `Expected the mock function to have returned ${expected} times, but it returned ${returned.length} times`,
      );
    }
  }
}
export function toHaveReturnedWith(
  context: MatcherContext,
  expected: unknown,
): MatchResult {
  const calls = getMockCalls(context.value);
  const returned = calls.filter((call) => call.returns);
  const returnedWithExpected = returned.some((call) =>
    equal(call.returned, expected)
  );

  if (context.isNot) {
    if (returnedWithExpected) {
      throw new AssertionError(
        `Expected the mock function to not have returned with ${
          inspectArg(expected)
        }, but it did`,
      );
    }
  } else {
    if (!returnedWithExpected) {
      throw new AssertionError(
        `Expected the mock function to have returned with ${
          inspectArg(expected)
        }, but it did not`,
      );
    }
  }
}

export function toHaveLastReturnedWith(
  context: MatcherContext,
  expected: unknown,
): MatchResult {
  const calls = getMockCalls(context.value);
  const returned = calls.filter((call) => call.returns);
  const lastReturnedWithExpected = returned.length > 0 &&
    equal(returned[returned.length - 1].returned, expected);

  if (context.isNot) {
    if (lastReturnedWithExpected) {
      throw new AssertionError(
        `Expected the mock function to not have last returned with ${
          inspectArg(expected)
        }, but it did`,
      );
    }
  } else {
    if (!lastReturnedWithExpected) {
      throw new AssertionError(
        `Expected the mock function to have last returned with ${
          inspectArg(expected)
        }, but it did not`,
      );
    }
  }
}

export function toHaveNthReturnedWith(
  context: MatcherContext,
  nth: number,
  expected: unknown,
): MatchResult {
  if (nth < 1) {
    throw new Error(`nth(${nth}) must be greater than 0`);
  }

  const calls = getMockCalls(context.value);
  const returned = calls.filter((call) => call.returns);
  const returnIndex = nth - 1;
  const maybeNthReturned = returned[returnIndex];
  const nthReturnedWithExpected = maybeNthReturned &&
    equal(maybeNthReturned.returned, expected);

  if (context.isNot) {
    if (nthReturnedWithExpected) {
      throw new AssertionError(
        `Expected the mock function to not have n-th (n=${nth}) returned with ${
          inspectArg(expected)
        }, but it did`,
      );
    }
  } else {
    if (!nthReturnedWithExpected) {
      throw new AssertionError(
        `Expected the mock function to have n-th (n=${nth}) returned with ${
          inspectArg(expected)
        }, but it did not`,
      );
    }
  }
}

export function toThrow<E extends Error = Error>(
  context: MatcherContext,
  // deno-lint-ignore no-explicit-any
  expected: new (...args: any[]) => E,
): MatchResult {
  if (typeof context.value === "function") {
    try {
      context.value = context.value();
    } catch (err) {
      context.value = err;
    }
  }
  if (context.isNot) {
    let isError = false;
    try {
      assertIsError(context.value, expected, undefined, context.customMessage);
      isError = true;
      throw new AssertionError(`Expected to NOT throw ${expected}`);
    } catch (e) {
      if (isError) {
        throw e;
      }
      return;
    }
  }
  return assertIsError(
    context.value,
    expected,
    undefined,
    context.customMessage,
  );
}
