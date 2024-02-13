// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Returns whether the given date or year (in number) is a leap year or not in the local time zone.
 * based on: https://docs.microsoft.com/en-us/office/troubleshoot/excel/determine-a-leap-year
 *
 * @example
 * ```ts
 * import { isLeap } from "https://deno.land/std@$STD_VERSION/datetime/is_leap.ts";
 *
 * isLeap(new Date("1970-01-02")); // => returns false
 * isLeap(new Date("1972-01-02")); // => returns true
 * isLeap(new Date("2000-01-02")); // => returns true
 * isLeap(new Date("2100-01-02")); // => returns false
 * isLeap(1972); // => returns true
 * ```
 *
 * Some dates may return different values depending on your timezone.
 *
 * @example
 * ```ts
 * import { isLeap } from "https://deno.land/std@$STD_VERSION/datetime/is_leap.ts";
 *
 * isLeap(new Date("2000-01-01")); // => returns true if the local timezone is GMT+0, returns false if the local timezone is GMT-1
 * isLeap(2000); // => returns true regardless of the local timezone
 * ```
 *
 * @param year year in number or Date format
 */
export function isLeap(year: Date | number): boolean {
  const yearNumber = year instanceof Date ? year.getFullYear() : year;
  return isYearNumberALeapYear(yearNumber);
}

/**
 * Returns whether the given date or year (in number) is a leap year or not in UTC time. This always returns the same value regardless of the local timezone.
 * based on: https://docs.microsoft.com/en-us/office/troubleshoot/excel/determine-a-leap-year
 *
 * @example
 * ```ts
 * import { isUtcLeap } from "https://deno.land/std@$STD_VERSION/datetime/is_leap.ts";
 *
 * isUtcLeap(2000); // => returns true regardless of the local timezone
 * isUtcLeap(new Date("2000-01-01")); // => returns true regardless of the local timezone
 * isUtcLeap(new Date("January 1, 2000 00:00:00 GMT+00:00")); // => returns true regardless of the local timezone
 * isUtcLeap(new Date("December 31, 2000 23:59:59 GMT+00:00")); // => returns true regardless of the local timezone
 * isUtcLeap(new Date("January 1, 2000 00:00:00 GMT+01:00")); // => returns false regardless of the local timezone
 * isUtcLeap(new Date("December 31, 2000 23:59:59 GMT-01:00")); // => returns false regardless of the local timezone
 * isUtcLeap(new Date("January 1, 2001 00:00:00 GMT+01:00")); // => returns true regardless of the local timezone
 * isUtcLeap(new Date("December 31, 1999 23:59:59 GMT-01:00")); // => returns true regardless of the local timezone
 * ```
 *
 * @param year year in number or Date format
 */
export function isUtcLeap(year: Date | number): boolean {
  const yearNumber = year instanceof Date ? year.getUTCFullYear() : year;
  return isYearNumberALeapYear(yearNumber);
}

function isYearNumberALeapYear(yearNumber: number): boolean {
  return (
    (yearNumber % 4 === 0 && yearNumber % 100 !== 0) || yearNumber % 400 === 0
  );
}
