// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode Date.toUTCString} instead.
 *
 * Formats the given date to IMF date time format. (Reference:
 * https://tools.ietf.org/html/rfc7231#section-7.1.1.1).
 * IMF is the time format to use when generating times in HTTP
 * headers. The time being formatted must be in UTC for Format to
 * generate the correct format.
 *
 * @example
 * ```ts
 * import { toIMF } from "https://deno.land/std@$STD_VERSION/datetime/to_imf.ts";
 *
 * toIMF(new Date(0)); // => returns "Thu, 01 Jan 1970 00:00:00 GMT"
 * ```
 * @param date Date to parse
 * @return IMF date formatted string
 */
export function toIMF(date: Date): string {
  function dtPad(v: string, lPad = 2): string {
    return v.padStart(lPad, "0");
  }
  const d = dtPad(date.getUTCDate().toString());
  const h = dtPad(date.getUTCHours().toString());
  const min = dtPad(date.getUTCMinutes().toString());
  const s = dtPad(date.getUTCSeconds().toString());
  const y = date.getUTCFullYear();
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const months = [
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
  ];
  return `${days[date.getUTCDay()]}, ${d} ${
    months[date.getUTCMonth()]
  } ${y} ${h}:${min}:${s} GMT`;
}
