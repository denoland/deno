// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Implements the CSV spec at https://tools.ietf.org/html/rfc4180

/** This module is browser compatible. */

const QUOTE = '"';
export const NEWLINE = "\r\n";

export class StringifyError extends Error {
  readonly name = "StringifyError";
}

function getEscapedString(value: unknown, sep: string): string {
  if (value === undefined || value === null) return "";
  let str = "";

  if (typeof value === "object") str = JSON.stringify(value);
  else str = String(value);

  // Is regex.test more performance here? If so, how to dynamically create?
  // https://stackoverflow.com/questions/3561493/
  if (str.includes(sep) || str.includes(NEWLINE) || str.includes(QUOTE)) {
    return `${QUOTE}${str.replaceAll(QUOTE, `${QUOTE}${QUOTE}`)}${QUOTE}`;
  }

  return str;
}

type PropertyAccessor = number | string;

/**
 * @param fn Optional callback for transforming the value
 *
 * @param header Explicit column header name. If omitted,
 * the (final) property accessor is used for this value.
 *
 * @param prop Property accessor(s) used to access the value on the object
 */
export type ColumnDetails = {
  // "unknown" is more type-safe, but inconvenient for user. How to resolve?
  // deno-lint-ignore no-explicit-any
  fn?: (value: any) => string | Promise<string>;
  header?: string;
  prop: PropertyAccessor | PropertyAccessor[];
};

export type Column = ColumnDetails | PropertyAccessor | PropertyAccessor[];

type NormalizedColumn = Omit<ColumnDetails, "header" | "prop"> & {
  header: string;
  prop: PropertyAccessor[];
};

function normalizeColumn(column: Column): NormalizedColumn {
  let fn: NormalizedColumn["fn"],
    header: NormalizedColumn["header"],
    prop: NormalizedColumn["prop"];

  if (typeof column === "object") {
    if (Array.isArray(column)) {
      header = String(column[column.length - 1]);
      prop = column;
    } else {
      ({ fn } = column);
      prop = Array.isArray(column.prop) ? column.prop : [column.prop];
      header = typeof column.header === "string"
        ? column.header
        : String(prop[prop.length - 1]);
    }
  } else {
    header = String(column);
    prop = [column];
  }

  return { fn, header, prop };
}

type ObjectWithStringPropertyKeys = Record<string, unknown>;

/** An object (plain or array) */
export type DataItem = ObjectWithStringPropertyKeys | unknown[];

/**
 * Returns an array of values from an object using the property accessors
 * (and optional transform function) in each column
 */
async function getValuesFromItem(
  item: DataItem,
  normalizedColumns: NormalizedColumn[],
): Promise<unknown[]> {
  const values: unknown[] = [];

  for (const column of normalizedColumns) {
    let value: unknown = item;

    for (const prop of column.prop) {
      if (typeof value !== "object" || value === null) continue;
      if (Array.isArray(value)) {
        if (typeof prop === "number") value = value[prop];
        else {
          throw new StringifyError('Property accessor is not of type "number"');
        }
      } // I think this assertion is safe. Confirm?
      else value = (value as ObjectWithStringPropertyKeys)[prop];
    }

    if (typeof column.fn === "function") value = await column.fn(value);
    values.push(value);
  }

  return values;
}

/**
 * @param headers Whether or not to include the row of headers.
 * Default: `true`
 *
 * @param separator Delimiter used to separate values. Examples:
 *  - `","` _comma_ (Default)
 *  - `"\t"` _tab_
 *  - `"|"` _pipe_
 *  - etc.
 */
export type StringifyOptions = {
  headers?: boolean;
  separator?: string;
};

/**
 * @param data The array of objects to encode
 * @param columns Array of values specifying which data to include in the output
 * @param options Output formatting options
 */
export async function stringify(
  data: DataItem[],
  columns: Column[],
  options: StringifyOptions = {},
): Promise<string> {
  const { headers, separator: sep } = {
    headers: true,
    separator: ",",
    ...options,
  };
  if (sep.includes(QUOTE) || sep.includes(NEWLINE)) {
    const message = [
      "Separator cannot include the following strings:",
      '  - U+0022: Quotation mark (")',
      "  - U+000D U+000A: Carriage Return + Line Feed (\\r\\n)",
    ].join("\n");
    throw new StringifyError(message);
  }

  const normalizedColumns = columns.map(normalizeColumn);
  let output = "";

  if (headers) {
    output += normalizedColumns
      .map((column) => getEscapedString(column.header, sep))
      .join(sep);
    output += NEWLINE;
  }

  for (const item of data) {
    const values = await getValuesFromItem(item, normalizedColumns);
    output += values
      .map((value) => getEscapedString(value, sep))
      .join(sep);
    output += NEWLINE;
  }

  return output;
}
