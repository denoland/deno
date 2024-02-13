// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/** Reads and writes comma-separated values (CSV) files.
 *
 * There are many kinds of CSV files; this module supports the format described
 * in [RFC 4180](https://www.rfc-editor.org/rfc/rfc4180.html).
 *
 * A csv file contains zero or more records of one or more fields per record.
 * Each record is separated by the newline character. The final record may
 * optionally be followed by a newline character.
 *
 * ```csv
 * field1,field2,field3
 * ```
 *
 * White space is considered part of a field.
 *
 * Carriage returns before newline characters are silently removed.
 *
 * Blank lines are ignored. A line with only whitespace characters (excluding
 * the ending newline character) is not considered a blank line.
 *
 * Fields which start and stop with the quote character " are called
 * quoted-fields. The beginning and ending quote are not part of the field.
 *
 * The source:
 *
 * ```csv
 * normal string,"quoted-field"
 * ```
 *
 * results in the fields
 *
 * ```ts
 * [`normal string`, `quoted-field`]
 * ```
 *
 * Within a quoted-field a quote character followed by a second quote character is considered a single quote.
 *
 * ```csv
 * "the ""word"" is true","a ""quoted-field"""
 * ```
 *
 * results in
 *
 * [`the "word" is true`, `a "quoted-field"`]
 *
 * Newlines and commas may be included in a quoted-field
 *
 * ```csv
 * "Multi-line
 * field","comma is ,"
 * ```
 *
 * results in
 *
 * ```ts
 * [`Multi-line
 * field`, `comma is ,`]
 * ```
 *
 * @module
 */

export * from "./stringify.ts";
export * from "./parse.ts";
export * from "./csv_parse_stream.ts";
export * from "./csv_stringify_stream.ts";
