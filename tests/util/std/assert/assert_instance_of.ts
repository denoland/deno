// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { AssertionError } from "./assertion_error.ts";

// deno-lint-ignore no-explicit-any
type AnyConstructor = new (...args: any[]) => any;
type GetConstructorType<T extends AnyConstructor> = T extends // deno-lint-ignore no-explicit-any
new (...args: any) => infer C ? C
  : never;

/**
 * Make an assertion that `obj` is an instance of `type`.
 * If not then throw.
 */
export function assertInstanceOf<T extends AnyConstructor>(
  actual: unknown,
  expectedType: T,
  msg = "",
): asserts actual is GetConstructorType<T> {
  if (actual instanceof expectedType) return;

  const msgSuffix = msg ? `: ${msg}` : ".";
  const expectedTypeStr = expectedType.name;

  let actualTypeStr = "";
  if (actual === null) {
    actualTypeStr = "null";
  } else if (actual === undefined) {
    actualTypeStr = "undefined";
  } else if (typeof actual === "object") {
    actualTypeStr = actual.constructor?.name ?? "Object";
  } else {
    actualTypeStr = typeof actual;
  }

  if (expectedTypeStr === actualTypeStr) {
    msg =
      `Expected object to be an instance of "${expectedTypeStr}"${msgSuffix}`;
  } else if (actualTypeStr === "function") {
    msg =
      `Expected object to be an instance of "${expectedTypeStr}" but was not an instanced object${msgSuffix}`;
  } else {
    msg =
      `Expected object to be an instance of "${expectedTypeStr}" but was "${actualTypeStr}"${msgSuffix}`;
  }

  throw new AssertionError(msg);
}
