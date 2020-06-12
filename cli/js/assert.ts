// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/* eslint-disable @typescript-eslint/no-explicit-any */
export interface AssertionErrorOptions {
  message?: string;
  actual?: any;
  expected?: any;
  operator?: string;
  stackStartFn?: Function;
}

export class AssertionError extends Error {
  actual?: any;
  expected?: any;
  operator?: string;

  constructor({
    message,
    actual,
    expected,
    operator,
    stackStartFn = AssertionError,
  }: AssertionErrorOptions = {}) {
    super(message);
    this.name = "AssertionError";
    this.actual = actual;
    this.expected = expected;
    this.operator = operator;
    Error.captureStackTrace(this, stackStartFn);
  }
}
/* eslint-enable */

export function assert(
  cond: unknown,
  message = "Assertion failed."
): asserts cond {
  if (!cond) {
    throw new AssertionError({ message, stackStartFn: assert });
  }
}
