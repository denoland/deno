// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { DateTimeFormatter } from "./formatter.ts";

export const SECOND = 1e3;
export const MINUTE = SECOND * 60;
export const HOUR = MINUTE * 60;
export const DAY = HOUR * 24;
export const WEEK = DAY * 7;
const DAYS_PER_WEEK = 7;

enum Day {
  Sun,
  Mon,
  Tue,
  Wed,
  Thu,
  Fri,
  Sat,
}

/**
 * Parse date from string using format string
 * @param dateString Date string
 * @param format Format string
 * @return Parsed date
 */
export function parse(dateString: string, formatString: string): Date {
  const formatter = new DateTimeFormatter(formatString);
  const parts = formatter.parseToParts(dateString);
  const sortParts = formatter.sortDateTimeFormatPart(parts);
  return formatter.partsToDate(sortParts);
}

/**
 * Format date using format string
 * @param date Date
 * @param format Format string
 * @return formatted date string
 */
export function format(date: Date, formatString: string): string {
  const formatter = new DateTimeFormatter(formatString);
  return formatter.format(date);
}

/**
 * Get number of the day in the year
 * @return Number of the day in year
 */
export function dayOfYear(date: Date): number {
  // Values from 0 to 99 map to the years 1900 to 1999. All other values are the actual year. (https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/Date)
  // Using setFullYear as a workaround

  const yearStart = new Date(date);

  yearStart.setUTCFullYear(date.getUTCFullYear(), 0, 0);
  const diff = date.getTime() -
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
    Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()),
  );

  const day = workingDate.getUTCDay();

  const nearestThursday = workingDate.getUTCDate() +
    Day.Thu -
    (day === Day.Sun ? DAYS_PER_WEEK : day);

  workingDate.setUTCDate(nearestThursday);

  // Get first day of year
  const yearStart = new Date(Date.UTC(workingDate.getUTCFullYear(), 0, 1));

  // return the calculated full weeks to nearest Thursday
  return Math.ceil((workingDate.getTime() - yearStart.getTime() + DAY) / WEEK);
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
  | "milliseconds"
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
  options?: DifferenceOptions,
): DifferenceFormat {
  const uniqueUnits = options?.units ? [...new Set(options?.units)] : [
    "milliseconds",
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
      case "milliseconds":
        differences.milliseconds = differenceInMs;
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
            calculateMonthsDifference(bigger, smaller) / 4,
        );
        break;
      case "years":
        differences.years = Math.floor(
          (typeof differences.months !== "undefined" &&
            differences.months / 12) ||
            calculateMonthsDifference(bigger, smaller) / 12,
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
    biggerDate.getMonth() - compareResult * calendarDiffrences,
  );
  const isLastMonthNotFull = biggerDate > smallerDate
    ? 1
    : -1 === -compareResult
    ? 1
    : 0;
  const months = compareResult * (calendarDiffrences - isLastMonthNotFull);
  return months === 0 ? 0 : months;
}
