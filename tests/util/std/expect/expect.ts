// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2019 Allain Lalonde. All rights reserved. ISC License.

import type { AnyConstructor, Matcher, MatcherContext } from "./_types.ts";
import { AssertionError } from "../assert/assertion_error.ts";
import {
  toBe,
  toBeCloseTo,
  toBeDefined,
  toBeFalsy,
  toBeGreaterThan,
  toBeGreaterThanOrEqual,
  toBeInstanceOf,
  toBeLessThan,
  toBeLessThanOrEqual,
  toBeNaN,
  toBeNull,
  toBeTruthy,
  toBeUndefined,
  toContain,
  toContainEqual,
  toEqual,
  toHaveBeenCalled,
  toHaveBeenCalledTimes,
  toHaveBeenCalledWith,
  toHaveBeenLastCalledWith,
  toHaveBeenNthCalledWith,
  toHaveLastReturnedWith,
  toHaveLength,
  toHaveNthReturnedWith,
  toHaveProperty,
  toHaveReturned,
  toHaveReturnedTimes,
  toHaveReturnedWith,
  toMatch,
  toMatchObject,
  toStrictEqual,
  toThrow,
} from "./_matchers.ts";
export interface Expected {
  lastCalledWith(...expected: unknown[]): void;
  lastReturnedWith(expected: unknown): void;
  nthCalledWith(nth: number, ...expected: unknown[]): void;
  nthReturnedWith(nth: number, expected: unknown): void;
  toBeCalled(): void;
  toBeCalledTimes(expected: number): void;
  toBeCalledWith(...expected: unknown[]): void;
  toBeCloseTo(candidate: number, tolerance?: number): void;
  toBeDefined(): void;
  toBeFalsy(): void;
  toBeGreaterThan(expected: number): void;
  toBeGreaterThanOrEqual(expected: number): void;
  toBeInstanceOf<T extends AnyConstructor>(expected: T): void;
  toBeLessThan(expected: number): void;
  toBeLessThanOrEqual(expected: number): void;
  toBeNaN(): void;
  toBeNull(): void;
  toBeTruthy(): void;
  toBeUndefined(): void;
  toBe(expected: unknown): void;
  toContainEqual(expected: unknown): void;
  toContain(expected: unknown): void;
  toEqual(expected: unknown): void;
  toHaveBeenCalledTimes(expected: number): void;
  toHaveBeenCalledWith(...expected: unknown[]): void;
  toHaveBeenCalled(): void;
  toHaveBeenLastCalledWith(...expected: unknown[]): void;
  toHaveBeenNthCalledWith(nth: number, ...expected: unknown[]): void;
  toHaveLength(expected: number): void;
  toHaveLastReturnedWith(expected: unknown): void;
  toHaveNthReturnedWith(nth: number, expected: unknown): void;
  toHaveProperty(propName: string | string[], value?: unknown): void;
  toHaveReturnedTimes(expected: number): void;
  toHaveReturnedWith(expected: unknown): void;
  toHaveReturned(): void;
  toMatch(expected: RegExp): void;
  toMatchObject(expected: Record<PropertyKey, unknown>): void;
  toReturn(): void;
  toReturnTimes(expected: number): void;
  toReturnWith(expected: unknown): void;
  toStrictEqual(candidate: unknown): void;
  // deno-lint-ignore no-explicit-any
  toThrow<E extends Error = Error>(expected?: new (...args: any[]) => E): void;
  not: Expected;
  resolves: Async<Expected>;
  rejects: Async<Expected>;
}

type MatcherKey = keyof Omit<Expected, "not" | "resolves" | "rejects">;

const matchers: Record<MatcherKey, Matcher> = {
  lastCalledWith: toHaveBeenLastCalledWith,
  lastReturnedWith: toHaveLastReturnedWith,
  nthCalledWith: toHaveBeenNthCalledWith,
  nthReturnedWith: toHaveNthReturnedWith,
  toBeCalled: toHaveBeenCalled,
  toBeCalledTimes: toHaveBeenCalledTimes,
  toBeCalledWith: toHaveBeenCalledWith,
  toBeCloseTo,
  toBeDefined,
  toBeFalsy,
  toBeGreaterThanOrEqual,
  toBeGreaterThan,
  toBeInstanceOf,
  toBeLessThanOrEqual,
  toBeLessThan,
  toBeNaN,
  toBeNull,
  toBeTruthy,
  toBeUndefined,
  toBe,
  toContainEqual,
  toContain,
  toEqual,
  toHaveBeenCalledTimes,
  toHaveBeenCalledWith,
  toHaveBeenCalled,
  toHaveBeenLastCalledWith,
  toHaveBeenNthCalledWith,
  toHaveLength,
  toHaveLastReturnedWith,
  toHaveNthReturnedWith,
  toHaveProperty,
  toHaveReturnedTimes,
  toHaveReturnedWith,
  toHaveReturned,
  toMatchObject,
  toMatch,
  toReturn: toHaveReturned,
  toReturnTimes: toHaveReturnedTimes,
  toReturnWith: toHaveReturnedWith,
  toStrictEqual,
  toThrow,
};

export function expect(value: unknown, customMessage?: string): Expected {
  let isNot = false;
  let isPromised = false;
  const self: Expected = new Proxy<Expected>(
    <Expected> {},
    {
      get(_, name) {
        if (name === "not") {
          isNot = !isNot;
          return self;
        }

        if (name === "resolves") {
          if (!isPromiseLike(value)) {
            throw new AssertionError("expected value must be Promiselike");
          }

          isPromised = true;
          return self;
        }

        if (name === "rejects") {
          if (!isPromiseLike(value)) {
            throw new AssertionError("expected value must be a PromiseLike");
          }

          value = value.then(
            (value) => {
              throw new AssertionError(
                `Promise did not reject. resolved to ${value}`,
              );
            },
            (err) => err,
          );
          isPromised = true;
          return self;
        }

        const matcher: Matcher = matchers[name as MatcherKey];
        if (!matcher) {
          throw new TypeError(
            typeof name === "string"
              ? `matcher not found: ${name}`
              : "matcher not found",
          );
        }

        return (...args: unknown[]) => {
          function applyMatcher(value: unknown, args: unknown[]) {
            const context: MatcherContext = {
              value,
              isNot: false,
              customMessage,
            };
            if (isNot) {
              context.isNot = true;
            }
            matcher(context, ...args);
          }

          return isPromised
            ? (value as Promise<unknown>).then((value: unknown) =>
              applyMatcher(value, args)
            )
            : applyMatcher(value, args);
        };
      },
    },
  );

  return self;
}

// a helper type to match any function. Used so that we only convert functions
// to return a promise and not properties.
type Fn = (...args: unknown[]) => unknown;

// converts all the methods in an interface to be async functions
export type Async<T> = {
  [K in keyof T]: T[K] extends Fn
    ? (...args: Parameters<T[K]>) => Promise<ReturnType<T[K]>>
    : T[K];
};

function isPromiseLike(value: unknown): value is PromiseLike<unknown> {
  if (value == null) {
    return false;
  } else {
    return typeof ((value as Record<string, unknown>).then) === "function";
  }
}
