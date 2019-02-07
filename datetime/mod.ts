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

  if (format === "mm-dd-yyyy") {
    const datePattern = /^(\d{2})-(\d{2})-(\d{4})$/;
    [, m, d, y] = datePattern.exec(dateStr);
  } else if (format === "dd-mm-yyyy") {
    const datePattern = /^(\d{2})-(\d{2})-(\d{4})$/;
    [, d, m, y] = datePattern.exec(dateStr);
  } else if (format === "yyyy-mm-dd") {
    const datePattern = /^(\d{4})-(\d{2})-(\d{2})$/;
    [, y, m, d] = datePattern.exec(dateStr);
  } else {
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

  if (format === "mm-dd-yyyy hh:mm") {
    const datePattern = /^(\d{2})-(\d{2})-(\d{4}) (\d{2}):(\d{2})$/;
    [, m, d, y, ho, mi] = datePattern.exec(datetimeStr);
  } else if (format === "dd-mm-yyyy hh:mm") {
    const datePattern = /^(\d{2})-(\d{2})-(\d{4}) (\d{2}):(\d{2})$/;
    [, d, m, y, ho, mi] = datePattern.exec(datetimeStr);
  } else if (format === "yyyy-mm-dd hh:mm") {
    const datePattern = /^(\d{4})-(\d{2})-(\d{2}) (\d{2}):(\d{2})$/;
    [, y, m, d, ho, mi] = datePattern.exec(datetimeStr);
  } else if (format === "hh:mm mm-dd-yyyy") {
    const datePattern = /^(\d{2}):(\d{2}) (\d{2})-(\d{2})-(\d{4})$/;
    [, ho, mi, m, d, y] = datePattern.exec(datetimeStr);
  } else if (format === "hh:mm dd-mm-yyyy") {
    const datePattern = /^(\d{2}):(\d{2}) (\d{2})-(\d{2})-(\d{4})$/;
    [, ho, mi, d, m, y] = datePattern.exec(datetimeStr);
  } else if (format === "hh:mm yyyy-mm-dd") {
    const datePattern = /^(\d{2}):(\d{2}) (\d{4})-(\d{2})-(\d{2})$/;
    [, ho, mi, y, m, d] = datePattern.exec(datetimeStr);
  } else {
    throw new Error("Invalid datetime format!");
  }

  return new Date(Number(y), Number(m) - 1, Number(d), Number(ho), Number(mi));
}

/**
 * Get number of current day in year
 *
 * @return {number} Number of current day in year
 */
export function currentDayOfYear(): number {
  return (
    Math.ceil(new Date().getTime() / 86400000) -
    Math.floor(
      new Date().setFullYear(new Date().getFullYear(), 0, 1) / 86400000
    )
  );
}
