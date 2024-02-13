// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { equal } from "./equal.ts";
import { format } from "./_format.ts";
import { AssertionError } from "./assertion_error.ts";
import { red } from "../fmt/colors.ts";
import { buildMessage, diff, diffstr } from "./_diff.ts";
import { CAN_NOT_DISPLAY } from "./_constants.ts";

/**
 * Make an assertion that `actual` and `expected` are equal, deeply. If not
 * deeply equal, then throw.
 *
 * Type parameter can be specified to ensure values under comparison have the same type.
 *
 * @example
 * ```ts
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * Deno.test("example", function (): void {
 *   assertEquals("world", "world");
 *   assertEquals({ hello: "world" }, { hello: "world" });
 * });
 * ```
 *
 * Note: formatter option is experimental and may be removed in the future.
 */
export function assertEquals<T>(
  actual: T,
  expected: T,
  msg?: string,
  options: { formatter?: (value: unknown) => string } = {},
) {
  if (equal(actual, expected)) {
    return;
  }
  const { formatter = format } = options;
  const msgSuffix = msg ? `: ${msg}` : ".";
  let message = `Values are not equal${msgSuffix}`;

  const actualString = formatter(actual);
  const expectedString = formatter(expected);
  try {
    const stringDiff = (typeof actual === "string") &&
      (typeof expected === "string");
    const diffResult = stringDiff
      ? diffstr(actual as string, expected as string)
      : diff(actualString.split("\n"), expectedString.split("\n"));
    const diffMsg = buildMessage(diffResult, { stringDiff }).join("\n");
    message = `${message}\n${diffMsg}`;
  } catch {
    message = `${message}\n${red(CAN_NOT_DISPLAY)} + \n\n`;
  }
  throw new AssertionError(message);
}
