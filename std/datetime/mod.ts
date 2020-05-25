// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../_util/assert.ts";

export type DateFormat = "mm-dd-yyyy" | "dd-mm-yyyy" | "yyyy-mm-dd";

function execForce(reg: RegExp, pat: string): RegExpExecArray {
  const v = reg.exec(pat);
  assert(v != null);
  return v;
}
/**
 * Parse date from string using format string
 * @param dateStr Date string
 * @param format Format string
 * @return Parsed date
 */
export function parseDate(dateStr: string, format: DateFormat): Date {
  let m, d, y: string;
  let datePattern: RegExp;

  switch (format) {
    case "mm-dd-yyyy":
      datePattern = /^(\d{2})-(\d{2})-(\d{4})$/;
      [, m, d, y] = execForce(datePattern, dateStr);
      break;
    case "dd-mm-yyyy":
      datePattern = /^(\d{2})-(\d{2})-(\d{4})$/;
      [, d, m, y] = execForce(datePattern, dateStr);
      break;
    case "yyyy-mm-dd":
      datePattern = /^(\d{4})-(\d{2})-(\d{2})$/;
      [, y, m, d] = execForce(datePattern, dateStr);
      break;
    default:
      throw new Error("Invalid date format!");
  }

  return new Date(Number(y), Number(m) - 1, Number(d));
}

export type DateTimeFormat =
  | "mm-dd-yyyy hh:mm"
  | "dd-mm-yyyy hh:mm"
  | "yyyy-mm-dd hh:mm"
  | "hh:mm mm-dd-yyyy"
  | "hh:mm dd-mm-yyyy"
  | "hh:mm yyyy-mm-dd";

/**
 * Parse date & time from string using format string
 * @param dateStr Date & time string
 * @param format Format string
 * @return Parsed date
 */
export function parseDateTime(
  datetimeStr: string,
  format: DateTimeFormat
): Date {
  let m, d, y, ho, mi: string;
  let datePattern: RegExp;

  switch (format) {
    case "mm-dd-yyyy hh:mm":
      datePattern = /^(\d{2})-(\d{2})-(\d{4}) (\d{2}):(\d{2})$/;
      [, m, d, y, ho, mi] = execForce(datePattern, datetimeStr);
      break;
    case "dd-mm-yyyy hh:mm":
      datePattern = /^(\d{2})-(\d{2})-(\d{4}) (\d{2}):(\d{2})$/;
      [, d, m, y, ho, mi] = execForce(datePattern, datetimeStr);
      break;
    case "yyyy-mm-dd hh:mm":
      datePattern = /^(\d{4})-(\d{2})-(\d{2}) (\d{2}):(\d{2})$/;
      [, y, m, d, ho, mi] = execForce(datePattern, datetimeStr);
      break;
    case "hh:mm mm-dd-yyyy":
      datePattern = /^(\d{2}):(\d{2}) (\d{2})-(\d{2})-(\d{4})$/;
      [, ho, mi, m, d, y] = execForce(datePattern, datetimeStr);
      break;
    case "hh:mm dd-mm-yyyy":
      datePattern = /^(\d{2}):(\d{2}) (\d{2})-(\d{2})-(\d{4})$/;
      [, ho, mi, d, m, y] = execForce(datePattern, datetimeStr);
      break;
    case "hh:mm yyyy-mm-dd":
      datePattern = /^(\d{2}):(\d{2}) (\d{4})-(\d{2})-(\d{2})$/;
      [, ho, mi, y, m, d] = execForce(datePattern, datetimeStr);
      break;
    default:
      throw new Error("Invalid datetime format!");
  }

  return new Date(Number(y), Number(m) - 1, Number(d), Number(ho), Number(mi));
}

/**
 * Get number of the day in the year
 * @return Number of the day in year
 */
export function dayOfYear(date: Date): number {
  const dayMs = 1000 * 60 * 60 * 24;
  const yearStart = new Date(date.getFullYear(), 0, 0);
  const diff =
    date.getTime() -
    yearStart.getTime() +
    (yearStart.getTimezoneOffset() - date.getTimezoneOffset()) * 60 * 1000;
  return Math.floor(diff / dayMs);
}

/**
 * Get number of current day in year
 * @return Number of current day in year
 */
export function currentDayOfYear(): number {
  return dayOfYear(new Date());
}

/**
 * Parse a date to return a IMF formated string date
 * RFC: https://tools.ietf.org/html/rfc7231#section-7.1.1.1
 * IMF is the time format to use when generating times in HTTP
 * headers. The time being formatted must be in UTC for Format to
 * generate the correct format.
 * @param date Date to parse
 * @return IMF date formated string
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
