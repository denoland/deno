// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
export type DateFormat = "mm-dd-yyyy" | "dd-mm-yyyy" | "yyyy-mm-dd";

/**
 * Parse date from string using format string
 *
 * @param {string} dateStr - date string
 * @param {DateFormat} format - format string
 * @return {Date} Parsed date
 */
export function parseDate(dateStr: string, format: DateFormat): Date {
  let m, d, y: string;
  let datePattern: RegExp;

  switch (format) {
    case "mm-dd-yyyy":
      datePattern = /^(\d{2})-(\d{2})-(\d{4})$/;
      [, m, d, y] = datePattern.exec(dateStr)!;
      break;
    case "dd-mm-yyyy":
      datePattern = /^(\d{2})-(\d{2})-(\d{4})$/;
      [, d, m, y] = datePattern.exec(dateStr)!;
      break;
    case "yyyy-mm-dd":
      datePattern = /^(\d{4})-(\d{2})-(\d{2})$/;
      [, y, m, d] = datePattern.exec(dateStr)!;
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
 *
 * @param {string} dateStr - date & time string
 * @param {DateTimeFormat} format - format string
 * @return {Date} Parsed date
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
      [, m, d, y, ho, mi] = datePattern.exec(datetimeStr)!;
      break;
    case "dd-mm-yyyy hh:mm":
      datePattern = /^(\d{2})-(\d{2})-(\d{4}) (\d{2}):(\d{2})$/;
      [, d, m, y, ho, mi] = datePattern.exec(datetimeStr)!;
      break;
    case "yyyy-mm-dd hh:mm":
      datePattern = /^(\d{4})-(\d{2})-(\d{2}) (\d{2}):(\d{2})$/;
      [, y, m, d, ho, mi] = datePattern.exec(datetimeStr)!;
      break;
    case "hh:mm mm-dd-yyyy":
      datePattern = /^(\d{2}):(\d{2}) (\d{2})-(\d{2})-(\d{4})$/;
      [, ho, mi, m, d, y] = datePattern.exec(datetimeStr)!;
      break;
    case "hh:mm dd-mm-yyyy":
      datePattern = /^(\d{2}):(\d{2}) (\d{2})-(\d{2})-(\d{4})$/;
      [, ho, mi, d, m, y] = datePattern.exec(datetimeStr)!;
      break;
    case "hh:mm yyyy-mm-dd":
      datePattern = /^(\d{2}):(\d{2}) (\d{4})-(\d{2})-(\d{2})$/;
      [, ho, mi, y, m, d] = datePattern.exec(datetimeStr)!;
      break;
    default:
      throw new Error("Invalid datetime format!");
  }

  return new Date(Number(y), Number(m) - 1, Number(d), Number(ho), Number(mi));
}

/**
 * Get number of the day in the year
 * @return {number} Number of the day in year
 */
export function dayOfYear(date: Date): any {
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
 *
 * @return {number} Number of current day in year
 */
export function currentDayOfYear(): number {
  return dayOfYear(new Date());
}
