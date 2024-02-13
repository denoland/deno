// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Utilities for dealing with {@linkcode Date} objects.
 *
 * The following symbols from
 * [unicode LDML](http://www.unicode.org/reports/tr35/tr35-dates.html#Date_Field_Symbol_Table)
 * are supported:
 *
 * - `yyyy` - numeric year.
 * - `yy` - 2-digit year.
 * - `M` - numeric month.
 * - `MM` - 2-digit month.
 * - `d` - numeric day.
 * - `dd` - 2-digit day.
 *
 * - `H` - numeric hour (0-23 hours).
 * - `HH` - 2-digit hour (00-23 hours).
 * - `h` - numeric hour (1-12 hours).
 * - `hh` - 2-digit hour (01-12 hours).
 * - `m` - numeric minute.
 * - `mm` - 2-digit minute.
 * - `s` - numeric second.
 * - `ss` - 2-digit second.
 * - `S` - 1-digit fractionalSecond.
 * - `SS` - 2-digit fractionalSecond.
 * - `SSS` - 3-digit fractionalSecond.
 *
 * - `a` - dayPeriod, either `AM` or `PM`.
 *
 * - `'foo'` - quoted literal.
 * - `./-` - unquoted literal.
 *
 * This module is browser compatible.
 *
 * @module
 */

export * from "./constants.ts";
export * from "./day_of_year.ts";
export * from "./difference.ts";
export * from "./format.ts";
export * from "./is_leap.ts";
export * from "./parse.ts";
export * from "./to_imf.ts";
export * from "./week_of_year.ts";
