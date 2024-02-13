// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { DateTimeFormatter } from "./_common.ts";

/**
 * Takes an input `string` and a `formatString` to parse to a `date`.
 *
 * @example
 * ```ts
 * import { parse } from "https://deno.land/std@$STD_VERSION/datetime/parse.ts";
 *
 * parse("20-01-2019", "dd-MM-yyyy"); // output : new Date(2019, 0, 20)
 * parse("2019-01-20", "yyyy-MM-dd"); // output : new Date(2019, 0, 20)
 * parse("20.01.2019", "dd.MM.yyyy"); // output : new Date(2019, 0, 20)
 * parse("01-20-2019 16:34", "MM-dd-yyyy HH:mm"); // output : new Date(2019, 0, 20, 16, 34)
 * parse("01-20-2019 04:34 PM", "MM-dd-yyyy hh:mm a"); // output : new Date(2019, 0, 20, 16, 34)
 * parse("16:34 01-20-2019", "HH:mm MM-dd-yyyy"); // output : new Date(2019, 0, 20, 16, 34)
 * parse("01-20-2019 16:34:23.123", "MM-dd-yyyy HH:mm:ss.SSS"); // output : new Date(2019, 0, 20, 16, 34, 23, 123)
 * ```
 *
 * @param dateString Date string
 * @param formatString Format string
 * @return Parsed date
 */
export function parse(dateString: string, formatString: string): Date {
  const formatter = new DateTimeFormatter(formatString);
  const parts = formatter.parseToParts(dateString);
  const sortParts = formatter.sortDateTimeFormatPart(parts);
  return formatter.partsToDate(sortParts);
}
