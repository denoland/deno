// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../_util/assert.ts";

export type DateFormat = "mm-dd-yyyy" | "dd-mm-yyyy" | "yyyy-mm-dd";

export const SECOND = 1e3;
export const MINUTE = SECOND * 60;
export const HOUR = MINUTE * 60;
export const DAY = HOUR * 24;
export const WEEK = DAY * 7;

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
  const yearStart = new Date(date.getFullYear(), 0, 0);
  const diff =
    date.getTime() -
    yearStart.getTime() +
    (yearStart.getTimezoneOffset() - date.getTimezoneOffset()) * 60 * 1000;
  return Math.floor(diff / DAY);
}

/**
 * Get number of the week in the year (ISO-8601)
 * @return Number of the week in year
 */
export function weekOfYear(date: Date): number {
  const workingDate = new Date(
    Date.UTC(date.getFullYear(), date.getMonth(), date.getDate())
  );

  // Set to nearest Thursday: current date + 4 - current day number
  // Make Sunday's day number 7
  workingDate.setUTCDate(
    workingDate.getUTCDate() + 4 - (workingDate.getUTCDay() || 7)
  );

  // Get first day of year
  const yearStart = new Date(Date.UTC(workingDate.getUTCFullYear(), 0, 1));

  // return the calculated full weeks to nearest Thursday
  return Math.ceil(
    ((workingDate.valueOf() - yearStart.valueOf()) / 86400000 + 1) / 7
  );
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

/**
 * Check given year is a leap year or not.
 * based on : https://docs.microsoft.com/en-us/office/troubleshoot/excel/determine-a-leap-year
 * @param year year in number or Date format
 */
export function isLeap(year: Date | number): boolean {
  const yearNumber = year instanceof Date ? year.getFullYear() : year;
  return (
    (yearNumber % 4 === 0 && yearNumber % 100 !== 0) || yearNumber % 400 === 0
  );
}

export type Unit =
  | "miliseconds"
  | "seconds"
  | "minutes"
  | "hours"
  | "days"
  | "weeks"
  | "months"
  | "quarters"
  | "years";

export type DifferenceFormat = Partial<Record<Unit, number>>;

export type DifferenceOptions = {
  units?: Unit[];
};

/**
 * Calculate difference between two dates.
 * @param from Year to calculate difference
 * @param to Year to calculate difference with
 * @param options Options for determining how to respond
 *
 * example :
 *
 * ```typescript
 * datetime.difference(new Date("2020/1/1"),new Date("2020/2/2"),{ units : ["days","months"] })
 * ```
 */
export function difference(
  from: Date,
  to: Date,
  options?: DifferenceOptions
): DifferenceFormat {
  const uniqueUnits = options?.units
    ? [...new Set(options?.units)]
    : [
        "miliseconds",
        "seconds",
        "minutes",
        "hours",
        "days",
        "weeks",
        "months",
        "quarters",
        "years",
      ];

  const bigger = Math.max(from.getTime(), to.getTime());
  const smaller = Math.min(from.getTime(), to.getTime());
  const differenceInMs = bigger - smaller;

  const differences: DifferenceFormat = {};

  for (const uniqueUnit of uniqueUnits) {
    switch (uniqueUnit) {
      case "miliseconds":
        differences.miliseconds = differenceInMs;
        break;
      case "seconds":
        differences.seconds = Math.floor(differenceInMs / SECOND);
        break;
      case "minutes":
        differences.minutes = Math.floor(differenceInMs / MINUTE);
        break;
      case "hours":
        differences.hours = Math.floor(differenceInMs / HOUR);
        break;
      case "days":
        differences.days = Math.floor(differenceInMs / DAY);
        break;
      case "weeks":
        differences.weeks = Math.floor(differenceInMs / WEEK);
        break;
      case "months":
        differences.months = calculateMonthsDifference(bigger, smaller);
        break;
      case "quarters":
        differences.quarters = Math.floor(
          (typeof differences.months !== "undefined" &&
            differences.months / 4) ||
            calculateMonthsDifference(bigger, smaller) / 4
        );
        break;
      case "years":
        differences.years = Math.floor(
          (typeof differences.months !== "undefined" &&
            differences.months / 12) ||
            calculateMonthsDifference(bigger, smaller) / 12
        );
        break;
    }
  }

  return differences;
}

function calculateMonthsDifference(bigger: number, smaller: number): number {
  const biggerDate = new Date(bigger);
  const smallerDate = new Date(smaller);
  const yearsDiff = biggerDate.getFullYear() - smallerDate.getFullYear();
  const monthsDiff = biggerDate.getMonth() - smallerDate.getMonth();
  const calendarDiffrences = Math.abs(yearsDiff * 12 + monthsDiff);
  const compareResult = biggerDate > smallerDate ? 1 : -1;
  biggerDate.setMonth(
    biggerDate.getMonth() - compareResult * calendarDiffrences
  );
  const isLastMonthNotFull =
    biggerDate > smallerDate ? 1 : -1 === -compareResult ? 1 : 0;
  const months = compareResult * (calendarDiffrences - isLastMonthNotFull);
  return months === 0 ? 0 : months;
}
