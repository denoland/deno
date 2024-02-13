// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

type PropertyAccessor = number | string;
type ObjectWithStringPropertyKeys = Record<string, unknown>;

/**
 * @param header Explicit column header name. If omitted,
 * the (final) property accessor is used for this value.
 *
 * @param prop Property accessor(s) used to access the value on the object
 */
export type ColumnDetails = {
  header?: string;
  prop: PropertyAccessor | PropertyAccessor[];
};

/**
 * The most essential aspect of a column is accessing the property holding the
 * data for that column on each object in the data array. If that member is at
 * the top level, `Column` can simply be a property accessor, which is either a
 * `string` (if it's a plain object) or a `number` (if it's an array).
 *
 * ```ts
 * const columns = [
 *   "name",
 * ];
 * ```
 *
 * Each property accessor will be used as the header for the column:
 *
 * | name |
 * | :--: |
 * | Deno |
 *
 * - If the required data is not at the top level (it's nested in other
 *   objects/arrays), then a simple property accessor won't work, so an array of
 *   them will be required.
 *
 *   ```ts
 *   const columns = [
 *     ["repo", "name"],
 *     ["repo", "org"],
 *   ];
 *   ```
 *
 *   When using arrays of property accessors, the header names inherit the value
 *   of the last accessor in each array:
 *
 *   | name |   org    |
 *   | :--: | :------: |
 *   | deno | denoland |
 *
 *  - If a different column header is desired, then a `ColumnDetails` object type
 *     can be used for each column:
 *
 *   - **`header?: string`** is the optional value to use for the column header
 *     name
 *
 *   - **`prop: PropertyAccessor | PropertyAccessor[]`** is the property accessor
 *     (`string` or `number`) or array of property accessors used to access the
 *     data on each object
 *
 *   ```ts
 *   const columns = [
 *     "name",
 *     {
 *       prop: ["runsOn", 0],
 *       header: "language 1",
 *     },
 *     {
 *       prop: ["runsOn", 1],
 *       header: "language 2",
 *     },
 *   ];
 *   ```
 *
 *   | name | language 1 | language 2 |
 *   | :--: | :--------: | :--------: |
 *   | Deno |    Rust    | TypeScript |
 */
export type Column = ColumnDetails | PropertyAccessor | PropertyAccessor[];

/** An object (plain or array) */
export type DataItem = ObjectWithStringPropertyKeys | unknown[];

export type StringifyOptions = {
  /** Whether to include the row of headers or not.
   *
   * @default {true}
   */
  headers?: boolean;
  /**
   * Delimiter used to separate values. Examples:
   *  - `","` _comma_
   *  - `"\t"` _tab_
   *  - `"|"` _pipe_
   *  - etc.
   *
   *  @default {","}
   */
  separator?: string;
  /**
   * a list of instructions for how to target and transform the data for each
   * column of output. This is also where you can provide an explicit header
   * name for the column.
   */
  columns?: Column[];
  /**
   * Whether to add a
   * [byte-order mark](https://en.wikipedia.org/wiki/Byte_order_mark) to the
   * beginning of the file content. Required by software such as MS Excel to
   * properly display Unicode text.
   *
   * @default {false}
   */
  bom?: boolean;
};

const QUOTE = '"';
const LF = "\n";
const CRLF = "\r\n";
const BYTE_ORDER_MARK = "\ufeff";

function getEscapedString(value: unknown, sep: string): string {
  if (value === undefined || value === null) return "";
  let str = "";

  if (typeof value === "object") str = JSON.stringify(value);
  else str = String(value);

  // Is regex.test more performant here? If so, how to dynamically create?
  // https://stackoverflow.com/questions/3561493/
  if (str.includes(sep) || str.includes(LF) || str.includes(QUOTE)) {
    return `${QUOTE}${str.replaceAll(QUOTE, `${QUOTE}${QUOTE}`)}${QUOTE}`;
  }

  return str;
}

type NormalizedColumn = Omit<ColumnDetails, "header" | "prop"> & {
  header: string;
  prop: PropertyAccessor[];
};

function normalizeColumn(column: Column): NormalizedColumn {
  let header: NormalizedColumn["header"],
    prop: NormalizedColumn["prop"];

  if (typeof column === "object") {
    if (Array.isArray(column)) {
      header = String(column[column.length - 1]);
      prop = column;
    } else {
      prop = Array.isArray(column.prop) ? column.prop : [column.prop];
      header = typeof column.header === "string"
        ? column.header
        : String(prop[prop.length - 1]);
    }
  } else {
    header = String(column);
    prop = [column];
  }

  return { header, prop };
}

export class StringifyError extends Error {
  override readonly name = "StringifyError";
}

/**
 * Returns an array of values from an object using the property accessors
 * (and optional transform function) in each column
 */
function getValuesFromItem(
  item: DataItem,
  normalizedColumns: NormalizedColumn[],
): unknown[] {
  const values: unknown[] = [];

  if (normalizedColumns.length) {
    for (const column of normalizedColumns) {
      let value: unknown = item;

      for (const prop of column.prop) {
        if (typeof value !== "object" || value === null) continue;
        if (Array.isArray(value)) {
          if (typeof prop === "number") value = value[prop];
          else {
            throw new StringifyError(
              'Property accessor is not of type "number"',
            );
          }
        } // I think this assertion is safe. Confirm?
        else value = (value as ObjectWithStringPropertyKeys)[prop];
      }

      values.push(value);
    }
  } else {
    if (Array.isArray(item)) {
      values.push(...item);
    } else if (typeof item === "object") {
      throw new StringifyError(
        "No property accessor function was provided for object",
      );
    } else {
      values.push(item);
    }
  }

  return values;
}

/**
 * Write data using CSV encoding.
 *
 * @param data The source data to stringify. It's an array of items which are
 * plain objects or arrays.
 *
 * `DataItem: Record<string, unknown> | unknown[]`
 *
 * ```ts
 * const data = [
 *   {
 *     name: "Deno",
 *     repo: { org: "denoland", name: "deno" },
 *     runsOn: ["Rust", "TypeScript"],
 *   },
 * ];
 * ```
 *
 * @example
 * ```ts
 * import {
 *   Column,
 *   stringify,
 * } from "https://deno.land/std@$STD_VERSION/csv/stringify.ts";
 *
 * type Character = {
 *   age: number;
 *   name: {
 *     first: string;
 *     last: string;
 *   };
 * };
 *
 * const data: Character[] = [
 *   {
 *     age: 70,
 *     name: {
 *       first: "Rick",
 *       last: "Sanchez",
 *     },
 *   },
 *   {
 *     age: 14,
 *     name: {
 *       first: "Morty",
 *       last: "Smith",
 *     },
 *   },
 * ];
 *
 * let columns: Column[] = [
 *   ["name", "first"],
 *   "age",
 * ];
 *
 * console.log(stringify(data, { columns }));
 * // first,age
 * // Rick,70
 * // Morty,14
 * ```
 *
 * @param options Output formatting options
 */
export function stringify(
  data: DataItem[],
  { headers = true, separator: sep = ",", columns = [], bom = false }:
    StringifyOptions = {},
): string {
  if (sep.includes(QUOTE) || sep.includes(CRLF)) {
    const message = [
      "Separator cannot include the following strings:",
      '  - U+0022: Quotation mark (")',
      "  - U+000D U+000A: Carriage Return + Line Feed (\\r\\n)",
    ].join("\n");
    throw new StringifyError(message);
  }

  const normalizedColumns = columns.map(normalizeColumn);
  let output = "";

  if (bom) {
    output += BYTE_ORDER_MARK;
  }

  if (headers) {
    output += normalizedColumns
      .map((column) => getEscapedString(column.header, sep))
      .join(sep);
    output += CRLF;
  }

  for (const item of data) {
    const values = getValuesFromItem(item, normalizedColumns);
    output += values
      .map((value) => getEscapedString(value, sep))
      .join(sep);
    output += CRLF;
  }

  return output;
}
