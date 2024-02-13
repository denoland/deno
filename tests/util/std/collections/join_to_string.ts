// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Options for joinToString
 */
export type JoinToStringOptions = {
  separator?: string;
  prefix?: string;
  suffix?: string;
  limit?: number;
  truncated?: string;
};

/**
 * Transforms the elements in the given array to strings using the given
 * selector. Joins the produced strings into one using the given `separator`
 * and applying the given `prefix` and `suffix` to the whole string afterwards.
 * If the array could be huge, you can specify a non-negative value of `limit`,
 * in which case only the first `limit` elements will be appended, followed by
 * the `truncated` string. Returns the resulting string.
 *
 * @example
 * ```ts
 * import { joinToString } from "https://deno.land/std@$STD_VERSION/collections/join_to_string.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const users = [
 *   { name: "Kim" },
 *   { name: "Anna" },
 *   { name: "Tim" },
 * ];
 *
 * const message = joinToString(users, (it) => it.name, {
 *   suffix: " are winners",
 *   prefix: "result: ",
 *   separator: " and ",
 *   limit: 1,
 *   truncated: "others",
 * });
 *
 * assertEquals(message, "result: Kim and others are winners");
 * ```
 */
export function joinToString<T>(
  array: Iterable<T>,
  selector: (el: T) => string,
  {
    separator = ",",
    prefix = "",
    suffix = "",
    limit = -1,
    truncated = "...",
  }: Readonly<JoinToStringOptions> = {},
): string {
  let result = "";

  let index = -1;
  for (const el of array) {
    index++;

    if (index > 0) {
      result += separator;
    }

    if (limit > -1 && index >= limit) {
      result += truncated;
      break;
    }

    result += selector(el);
  }

  result = prefix + result + suffix;

  return result;
}
